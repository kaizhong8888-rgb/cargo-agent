//! 通用连接池模块
//!
//! 基于 Tokio 的通用连接池实现。
//! 支持：
//! - 最大连接数限制
//! - 空闲连接超时回收
//! - 连接健康检查
//! - 可配置的连接创建/销毁策略

use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time;
use tracing::{debug, error, warn};

/// 连接工厂 trait：负责创建和销毁连接
#[async_trait::async_trait]
pub trait ConnectionFactory: Send + Sync + 'static {
    /// 连接类型
    type Connection: Send + 'static;

    /// 创建新连接
    async fn create(&self) -> Result<Self::Connection>;

    /// 销毁连接（可选）
    async fn destroy(&self, _conn: Self::Connection) {
        drop(_conn);
    }

    /// 健康检查（可选）
    async fn health_check(&self, _conn: &Self::Connection) -> Result<()> {
        Ok(())
    }
}

/// 池化连接
struct PooledConnectionInner<C> {
    conn: C,
    created_at: Instant,
    last_used: Instant,
}

/// 连接池
pub struct ConnectionPool<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    factory: Arc<F>,
    /// 空闲连接队列
    idle: Arc<Mutex<VecDeque<PooledConnectionInner<C>>>>,
    /// 当前总连接数（空闲 + 活跃）
    total_count: Arc<Mutex<usize>>,
    /// 最大连接数
    max_size: usize,
    /// 空闲超时（超过此时间的连接将被回收）
    idle_timeout: Duration,
    /// 连接最大生命周期
    max_lifetime: Duration,
}

/// 从池中借出的连接
pub struct PooledConnection<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    inner: Option<PooledConnectionInner<C>>,
    pool: ConnectionPool<C, F>,
}

impl<C, F> std::ops::Deref for PooledConnection<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner.as_ref().unwrap().conn
    }
}

impl<C, F> std::ops::DerefMut for PooledConnection<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.as_mut().unwrap().conn
    }
}

impl<C, F> Drop for PooledConnection<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.return_connection(inner).await;
            });
        }
    }
}

impl<C, F> ConnectionPool<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    /// 创建新的连接池
    pub fn new(factory: F, max_size: usize) -> Self {
        Self {
            factory: Arc::new(factory),
            idle: Arc::new(Mutex::new(VecDeque::new())),
            total_count: Arc::new(Mutex::new(0)),
            max_size,
            idle_timeout: Duration::from_secs(300), // 5 分钟
            max_lifetime: Duration::from_secs(3600), // 1 小时
        }
    }

    /// 设置空闲连接超时
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// 设置连接最大生命周期
    pub fn with_max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = lifetime;
        self
    }

    /// 从池中获取连接
    pub async fn get(&self) -> Result<PooledConnection<C, F>> {
        // 尝试从空闲队列获取
        {
            let mut idle = self.idle.lock().await;
            while let Some(inner) = idle.pop_front() {
                // 检查连接是否过期
                if inner.created_at.elapsed() > self.max_lifetime {
                    // 连接过期，销毁
                    let _ = self.factory.destroy(inner.conn).await;
                    let mut count = self.total_count.lock().await;
                    *count = count.saturating_sub(1);
                    continue;
                }

                // 健康检查
                match self.factory.health_check(&inner.conn).await {
                    Ok(_) => {
                        debug!("从连接池获取连接（来自空闲队列）");
                        return Ok(PooledConnection {
                            inner: Some(inner),
                            pool: self.clone(),
                        });
                    }
                    Err(e) => {
                        warn!("连接健康检查失败，销毁: {}", e);
                        let _ = self.factory.destroy(inner.conn).await;
                        let mut count = self.total_count.lock().await;
                        *count = count.saturating_sub(1);
                    }
                }
            }
        }

        // 检查是否可以创建新连接
        {
            let mut count = self.total_count.lock().await;
            if *count >= self.max_size {
                // 池已满，等待并重试
                // 简单策略：短暂等待后返回错误（实际生产环境可能使用等待队列）
                tokio::time::sleep(Duration::from_millis(100)).await;
                return Err(anyhow::anyhow!(
                    "连接池已满 (max={})，请稍后重试",
                    self.max_size
                ));
            }
            *count += 1;
        }

        // 创建新连接
        debug!("创建新连接（池使用中）");
        let conn = self.factory.create().await.map_err(|e| {
            let mut count = self.total_count.lock().await;
            *count = count.saturating_sub(1);
            e
        })?;

        let now = Instant::now();
        Ok(PooledConnection {
            inner: Some(PooledConnectionInner {
                conn,
                created_at: now,
                last_used: now,
            }),
            pool: self.clone(),
        })
    }

    /// 归还连接到池
    async fn return_connection(&self, mut inner: PooledConnectionInner<C>) {
        inner.last_used = Instant::now();

        // 检查连接是否过期
        if inner.created_at.elapsed() > self.max_lifetime {
            debug!("连接生命周期已到，销毁");
            let _ = self.factory.destroy(inner.conn).await;
            let mut count = self.total_count.lock().await;
            *count = count.saturating_sub(1);
            return;
        }

        // 归还到空闲队列
        let mut idle = self.idle.lock().await;
        idle.push_back(inner);
        debug!("连接归还到池，空闲连接数: {}", idle.len());
    }

    /// 启动空闲连接回收任务
    pub async fn start_reaper(&self) {
        let idle = self.idle.clone();
        let total_count = self.total_count.clone();
        let idle_timeout = self.idle_timeout;
        let max_lifetime = self.max_lifetime;
        let factory = self.factory.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let mut idle_lock = idle.lock().await;
                let before = idle_lock.len();
                idle_lock.retain(|inner| {
                    let is_valid = inner.created_at.elapsed() <= max_lifetime
                        && inner.last_used.elapsed() <= idle_timeout;
                    if !is_valid {
                        debug!("回收过期连接");
                    }
                    is_valid
                });
                let after = idle_lock.len();
                let reaped = before - after;
                if reaped > 0 {
                    debug!("回收了 {} 个空闲连接", reaped);
                    let mut count = total_count.lock().await;
                    *count = count.saturating_sub(reaped);
                }
            }
        });
    }

    /// 获取当前空闲连接数
    pub async fn idle_count(&self) -> usize {
        self.idle.lock().await.len()
    }

    /// 获取当前总连接数
    pub async fn total_count(&self) -> usize {
        *self.total_count.lock().await
    }
}

/// 克隆连接池（增加引用计数）
impl<C, F> Clone for ConnectionPool<C, F>
where
    F: ConnectionFactory<Connection = C>,
{
    fn clone(&self) -> Self {
        Self {
            factory: self.factory.clone(),
            idle: self.idle.clone(),
            total_count: self.total_count.clone(),
            max_size: self.max_size,
            idle_timeout: self.idle_timeout,
            max_lifetime: self.max_lifetime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyFactory;

    #[async_trait::async_trait]
    impl ConnectionFactory for DummyFactory {
        type Connection = String;

        async fn create(&self) -> Result<String> {
            Ok("test_connection".to_string())
        }
    }

    #[tokio::test]
    async fn test_pool_create_and_get() {
        let pool = ConnectionPool::new(DummyFactory, 10);
        let conn = pool.get().await.unwrap();
        assert_eq!(*conn, "test_connection");
        assert_eq!(pool.total_count().await, 1);
        drop(conn);
        // 归还后，空闲连接数应为 1
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pool.idle_count().await, 1);
    }

    #[tokio::test]
    async fn test_pool_max_size() {
        let pool = ConnectionPool::new(DummyFactory, 2);
        let _c1 = pool.get().await.unwrap();
        let _c2 = pool.get().await.unwrap();

        // 池已满，应该出错
        let result = pool.get().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_reuse_connection() {
        let pool = ConnectionPool::new(DummyFactory, 10);
        {
            let _conn = pool.get().await.unwrap();
        }
        // 等待归还
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(pool.idle_count().await, 1);
        assert_eq!(pool.total_count().await, 1);

        // 再次获取应复用空闲连接
        let _conn = pool.get().await.unwrap();
        assert_eq!(pool.idle_count().await, 0);
        assert_eq!(pool.total_count().await, 1);
    }
}
