use std::{future::Future, net::SocketAddr, time::Duration};
use task_manager_api::db;
use task_manager_api::{self as api};

/// Spin up a fresh server on a random port, run the closure, then shut down.
async fn with_server<F, Fut>(f: F)
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = ()>,
{
    // In-memory SQLite database – each test gets a clean state.
    let pool = db::create_pool("sqlite::memory:").await.unwrap();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bind_addr = listener.local_addr().unwrap();

    // Spawn the server in the background.
    let app = api::build_router(pool);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base_url = format!("http://{bind_addr}");

    // Wait until the server is ready (with timeout).
    wait_for_server(&base_url, 10).await;

    f(base_url).await;
}

/// Retry GET /health until the server responds (or max attempts reached).
async fn wait_for_server(base_url: &str, max_retries: u32) {
    let client = reqwest::Client::new();
    for attempt in 0..max_retries {
        if let Ok(resp) = client
            .get(format!("{base_url}/health"))
            .timeout(Duration::from_millis(100))
            .send()
            .await
        {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(50 * (attempt + 1) as u64)).await;
    }
}

#[tokio::test]
async fn test_health_check() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
    })
    .await;
}

#[tokio::test]
async fn test_create_and_get_task() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        // Create
        let create_resp = client
            .post(format!("{base_url}/api/tasks"))
            .json(&serde_json::json!({
                "title": "Buy groceries",
                "description": "Milk, eggs, bread",
                "priority": 2,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(create_resp.status(), 201);

        let task: serde_json::Value = create_resp.json().await.unwrap();
        let id = task["id"].as_str().unwrap().to_owned();
        assert_eq!(task["title"], "Buy groceries");
        assert_eq!(task["description"], "Milk, eggs, bread");
        assert_eq!(task["priority"], 2);
        assert_eq!(task["status"], "todo");

        // Get by id
        let get_resp = client
            .get(format!("{base_url}/api/tasks/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(get_resp.status(), 200);

        let fetched: serde_json::Value = get_resp.json().await.unwrap();
        assert_eq!(fetched["id"], id);
    })
    .await;
}

#[tokio::test]
async fn test_list_tasks_with_pagination() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        // Create 5 tasks
        for i in 1..=5 {
            client
                .post(format!("{base_url}/api/tasks"))
                .json(&serde_json::json!({
                    "title": format!("Task {i}"),
                    "priority": i,
                }))
                .send()
                .await
                .unwrap();
        }

        // List with pagination
        let list_resp = client
            .get(format!("{base_url}/api/tasks?per_page=3&page=1"))
            .send()
            .await
            .unwrap();
        assert_eq!(list_resp.status(), 200);

        let body: serde_json::Value = list_resp.json().await.unwrap();
        assert_eq!(body["data"].as_array().unwrap().len(), 3);
        assert_eq!(body["total"], 5);
        assert_eq!(body["total_pages"], 2);
    })
    .await;
}

#[tokio::test]
async fn test_update_task() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        // Create
        let create_resp = client
            .post(format!("{base_url}/api/tasks"))
            .json(&serde_json::json!({"title": "Learn Rust"}))
            .send()
            .await
            .unwrap();
        let task: serde_json::Value = create_resp.json().await.unwrap();
        let id = task["id"].as_str().unwrap().to_owned();
        assert_eq!(task["status"], "todo");

        // Update status → in_progress
        let update_resp = client
            .patch(format!("{base_url}/api/tasks/{id}"))
            .json(&serde_json::json!({
                "status": "in_progress",
                "priority": 5,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(update_resp.status(), 200);

        let updated: serde_json::Value = update_resp.json().await.unwrap();
        assert_eq!(updated["status"], "in_progress");
        assert_eq!(updated["priority"], 5);
    })
    .await;
}

#[tokio::test]
async fn test_delete_task() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        // Create
        let create_resp = client
            .post(format!("{base_url}/api/tasks"))
            .json(&serde_json::json!({"title": "Delete me"}))
            .send()
            .await
            .unwrap();
        let task: serde_json::Value = create_resp.json().await.unwrap();
        let id = task["id"].as_str().unwrap().to_owned();

        // Delete
        let del_resp = client
            .delete(format!("{base_url}/api/tasks/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(del_resp.status(), 204);

        // Verify deletion
        let get_resp = client
            .get(format!("{base_url}/api/tasks/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(get_resp.status(), 404);
    })
    .await;
}

#[tokio::test]
async fn test_filter_tasks_by_status() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        // Create two tasks, mark one as done
        let r1 = client
            .post(format!("{base_url}/api/tasks"))
            .json(&serde_json::json!({"title": "Task A"}))
            .send()
            .await
            .unwrap();
        let task_a: serde_json::Value = r1.json().await.unwrap();
        let id_a = task_a["id"].as_str().unwrap().to_owned();

        let _r2 = client
            .post(format!("{base_url}/api/tasks"))
            .json(&serde_json::json!({"title": "Task B"}))
            .send()
            .await
            .unwrap();

        // Mark A as done
        client
            .patch(format!("{base_url}/api/tasks/{id_a}"))
            .json(&serde_json::json!({"status": "done"}))
            .send()
            .await
            .unwrap();

        // Filter by status=done
        let list_resp = client
            .get(format!("{base_url}/api/tasks?status=done"))
            .send()
            .await
            .unwrap();
        let body: serde_json::Value = list_resp.json().await.unwrap();
        assert_eq!(body["total"], 1);
        assert_eq!(body["data"][0]["title"], "Task A");
    })
    .await;
}

#[tokio::test]
async fn test_search_tasks() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        let tasks = vec!["Rust programming", "Python scripting", "Data analysis"];
        for title in &tasks {
            client
                .post(format!("{base_url}/api/tasks"))
                .json(&serde_json::json!({"title": title}))
                .send()
                .await
                .unwrap();
        }

        // Search for "rust"
        let resp = client
            .get(format!("{base_url}/api/tasks?search=rust"))
            .send()
            .await
            .unwrap();
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["total"], 1, "Should find 1 task matching 'rust'");
        let titles: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect();
        assert!(titles.contains(&"Rust programming"));
    })
    .await;
}

#[tokio::test]
async fn test_empty_update_fails() {
    with_server(|base_url| async move {
        let client = reqwest::Client::new();

        // Create
        let r = client
            .post(format!("{base_url}/api/tasks"))
            .json(&serde_json::json!({"title": "Test"}))
            .send()
            .await
            .unwrap();
        let task: serde_json::Value = r.json().await.unwrap();
        let id = task["id"].as_str().unwrap().to_owned();

        // Send empty PATCH
        let resp = client
            .patch(format!("{base_url}/api/tasks/{id}"))
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);

        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"]["code"], "bad_request");
    })
    .await;
}
