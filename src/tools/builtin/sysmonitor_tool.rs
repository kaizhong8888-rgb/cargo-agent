//! System Monitor Tool: query system resource usage.
//! Reports CPU, memory, disk, network, and process info.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use sysinfo::{CpuRefreshKind, Disks, Networks, System};

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(SysMonitorTool));
}

struct SysMonitorTool;

#[async_trait::async_trait]
impl Tool for SysMonitorTool {
    fn name(&self) -> &str {
        "sysmonitor"
    }

    fn description(&self) -> &str {
        "Query system resource usage. Actions: info, cpu, memory, disk, network, processes, all."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            tp(
                "action",
                "Action: info, cpu, memory, disk, network, processes, all",
                true,
            ),
            tp(
                "process_count",
                "Number of top processes (default: 10)",
                false,
            ),
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let pc = params
            .get("process_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        match action {
            "info" => Ok(sys_info()),
            "cpu" => Ok(cpu_info()),
            "memory" => Ok(memory_info()),
            "disk" => Ok(disk_info()),
            "network" => Ok(network_info()),
            "processes" => Ok(process_info(pc)),
            "all" => Ok(serde_json::json!({
                "system": sys_info(), "cpu": cpu_info(), "memory": memory_info(),
                "disk": disk_info(), "network": network_info(), "processes": process_info(pc),
            })),
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

fn tp(n: &str, d: &str, r: bool) -> ToolParameter {
    ToolParameter {
        name: n.into(),
        description: d.into(),
        required: r,
        parameter_type: "string".into(),
    }
}

fn sys_info() -> Value {
    let hostname = System::host_name().unwrap_or_default();
    let kernel = System::kernel_version().unwrap_or_default();
    let uptime = System::uptime();
    serde_json::json!({
        "hostname": hostname,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "kernel_version": kernel,
        "uptime_seconds": uptime,
        "uptime": fmt_duration(uptime),
        "name": System::name().unwrap_or_default(),
        "long_os_version": System::long_os_version().unwrap_or_default(),
    })
}

fn cpu_info() -> Value {
    let mut sys = System::new();
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());

    let cpus: Vec<Value> = sys
        .cpus()
        .iter()
        .map(|cpu| {
            serde_json::json!({
                "brand": cpu.brand(),
                "frequency_mhz": cpu.frequency(),
                "usage_percent": cpu.cpu_usage(),
                "name": cpu.name(),
                "vendor_id": cpu.vendor_id(),
            })
        })
        .collect();

    serde_json::json!({
        "total_cores": sys.cpus().len(),
        "global_usage_percent": sys.global_cpu_usage(),
        "cpus": cpus,
    })
}

fn memory_info() -> Value {
    let mut sys = System::new();
    sys.refresh_memory();
    let (tr, ur, fr) = (sys.total_memory(), sys.used_memory(), sys.free_memory());
    let (ts, us, fs) = (sys.total_swap(), sys.used_swap(), sys.free_swap());
    serde_json::json!({
        "ram": {
            "total_bytes": tr, "total_display": fmt_size(tr),
            "used_bytes": ur, "used_display": fmt_size(ur),
            "free_bytes": fr, "free_display": fmt_size(fr),
            "usage_percent": pct(ur, tr),
        },
        "swap": {
            "total_bytes": ts, "total_display": fmt_size(ts),
            "used_bytes": us, "used_display": fmt_size(us),
            "free_bytes": fs, "free_display": fmt_size(fs),
            "usage_percent": if ts > 0 { pct(us, ts) } else { "N/A".into() },
        },
    })
}

fn disk_info() -> Value {
    let disks = Disks::new_with_refreshed_list();
    let list: Vec<Value> = disks
        .iter()
        .map(|d| {
            let total = d.total_space();
            let avail = d.available_space();
            let used = total.saturating_sub(avail);
            serde_json::json!({
                "name": d.name().to_string_lossy(),
                "mount_point": d.mount_point().to_string_lossy(),
                "file_system": d.file_system().to_string_lossy().to_string(),
                "total_bytes": total, "total_display": fmt_size(total),
                "used_bytes": used, "used_display": fmt_size(used),
                "available_bytes": avail, "available_display": fmt_size(avail),
                "usage_percent": pct(used, total),
                "removable": d.is_removable(),
            })
        })
        .collect();

    let (ts, ta) = (
        disks.iter().map(|d| d.total_space()).sum::<u64>(),
        disks.iter().map(|d| d.available_space()).sum::<u64>(),
    );
    serde_json::json!({
        "total_space_bytes": ts, "total_space_display": fmt_size(ts),
        "total_used_bytes": ts.saturating_sub(ta), "total_used_display": fmt_size(ts.saturating_sub(ta)),
        "total_available_bytes": ta, "total_available_display": fmt_size(ta),
        "disks": list,
    })
}

fn network_info() -> Value {
    let nets = Networks::new_with_refreshed_list();
    let list: Vec<Value> = nets.iter().map(|(name, d)| {
        serde_json::json!({
            "interface": name,
            "received_bytes": d.total_received(), "received_display": fmt_size(d.total_received()),
            "transmitted_bytes": d.total_transmitted(), "transmitted_display": fmt_size(d.total_transmitted()),
        })
    }).collect();

    let (rx, tx) = (
        nets.values().map(|d| d.total_received()).sum::<u64>(),
        nets.values().map(|d| d.total_transmitted()).sum::<u64>(),
    );
    serde_json::json!({
        "total_received_bytes": rx, "total_received_display": fmt_size(rx),
        "total_transmitted_bytes": tx, "total_transmitted_display": fmt_size(tx),
        "interfaces": list,
    })
}

fn process_info(count: usize) -> Value {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, false);

    let mut procs: Vec<Value> = sys
        .processes()
        .iter()
        .map(|(pid, p)| {
            serde_json::json!({
                "pid": pid.as_u32(),
                "name": p.name().to_string_lossy(),
                "cpu_percent": p.cpu_usage(),
                "memory_bytes": p.memory(),
                "memory_display": fmt_size(p.memory()),
                "status": format!("{:?}", p.status()),
            })
        })
        .collect();

    procs.sort_by(|a, b| {
        b["cpu_percent"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["cpu_percent"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    procs.truncate(count);

    serde_json::json!({ "total_processes": sys.processes().len(), "top_processes": procs })
}

fn pct(part: u64, total: u64) -> String {
    if total > 0 {
        format!("{:.1}", (part as f64 / total as f64) * 100.0)
    } else {
        "0".into()
    }
}

fn fmt_duration(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    format!("{d}d {h}h {m}m")
}

fn fmt_size(bytes: u64) -> String {
    const U: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut s = bytes as f64;
    let mut i = 0;
    while s >= 1024.0 && i < 4 {
        s /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, U[i])
    } else {
        format!("{:.2} {}", s, U[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- pct ----

    #[test]
    fn test_pct_full() {
        assert_eq!(pct(100, 100), "100.0");
    }

    #[test]
    fn test_pct_half() {
        assert_eq!(pct(50, 100), "50.0");
    }

    #[test]
    fn test_pct_zero_part() {
        assert_eq!(pct(0, 100), "0.0");
    }

    #[test]
    fn test_pct_zero_total() {
        assert_eq!(pct(50, 0), "0");
    }

    #[test]
    fn test_pct_large() {
        assert_eq!(pct(8_589_934_592, 17_179_869_184), "50.0");
    }

    #[test]
    fn test_pct_rounding() {
        assert_eq!(pct(1, 3), "33.3");
        assert_eq!(pct(2, 3), "66.7");
    }

    // ---- fmt_duration ----

    #[test]
    fn test_fmt_duration_zero() {
        assert_eq!(fmt_duration(0), "0d 0h 0m");
    }

    #[test]
    fn test_fmt_duration_one_min() {
        assert_eq!(fmt_duration(60), "0d 0h 1m");
    }

    #[test]
    fn test_fmt_duration_one_hour() {
        assert_eq!(fmt_duration(3600), "0d 1h 0m");
    }

    #[test]
    fn test_fmt_duration_one_day() {
        assert_eq!(fmt_duration(86400), "1d 0h 0m");
    }

    #[test]
    fn test_fmt_duration_complex() {
        assert_eq!(fmt_duration(186300), "2d 3h 45m");
    }

    #[test]
    fn test_fmt_duration_leap_year() {
        assert!(fmt_duration(366 * 86400).contains("366d"));
    }

    #[test]
    fn test_fmt_duration_max() {
        let r = fmt_duration(u64::MAX);
        assert!(r.contains('d') && r.contains('h') && r.contains('m'));
    }

    // ---- fmt_size ----

    #[test]
    fn test_fmt_size_zero() {
        assert_eq!(fmt_size(0), "0 B");
    }

    #[test]
    fn test_fmt_size_bytes() {
        assert_eq!(fmt_size(512), "512 B");
    }

    #[test]
    fn test_fmt_size_just_below_kb() {
        assert_eq!(fmt_size(1023), "1023 B");
    }

    #[test]
    fn test_fmt_size_kb() {
        assert_eq!(fmt_size(1024), "1.00 KB");
    }

    #[test]
    fn test_fmt_size_just_above_kb() {
        assert_eq!(fmt_size(1025), "1.00 KB");
    }

    #[test]
    fn test_fmt_size_mb() {
        assert_eq!(fmt_size(1_048_576), "1.00 MB");
    }

    #[test]
    fn test_fmt_size_partial_mb() {
        assert_eq!(fmt_size(2_621_440), "2.50 MB");
    }

    #[test]
    fn test_fmt_size_gb() {
        assert_eq!(fmt_size(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn test_fmt_size_tb() {
        assert_eq!(fmt_size(1_099_511_627_776), "1.00 TB");
    }

    #[test]
    fn test_fmt_size_5tb() {
        assert_eq!(fmt_size(5 * 1_099_511_627_776), "5.00 TB");
    }

    #[test]
    fn test_fmt_size_boundary() {
        assert_eq!(fmt_size(1024), "1.00 KB");
        assert_eq!(fmt_size(1023), "1023 B");
    }

    // ---- Integration tests ----

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("invalid"));
        assert!(tool.execute(&params).await.is_err());
    }

    #[tokio::test]
    async fn test_missing_action() {
        let tool = SysMonitorTool;
        let params = HashMap::new();
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_info_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("info"));
        let r = tool.execute(&params).await.unwrap();
        assert!(r.get("hostname").is_some());
        assert!(r.get("os").is_some());
        assert!(r.get("uptime_seconds").is_some());
    }

    #[tokio::test]
    async fn test_cpu_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("cpu"));
        let r = tool.execute(&params).await.unwrap();
        assert!(r["total_cores"].as_u64().unwrap() > 0);
        assert!(r.get("global_usage_percent").is_some());
        assert!(!r["cpus"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_memory_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("memory"));
        let r = tool.execute(&params).await.unwrap();
        assert!(r["ram"]["total_bytes"].as_u64().unwrap() > 0);
        assert!(r["ram"]["usage_percent"].as_str().is_some());
        assert!(r["swap"].get("total_bytes").is_some());
    }

    #[tokio::test]
    async fn test_disk_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("disk"));
        let r = tool.execute(&params).await.unwrap();
        let disks = r["disks"].as_array().unwrap();
        assert!(!disks.is_empty());
        for d in disks {
            assert!(d.get("mount_point").is_some());
            assert!(d.get("total_bytes").is_some());
        }
    }

    #[tokio::test]
    async fn test_network_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("network"));
        let r = tool.execute(&params).await.unwrap();
        assert!(r.get("interfaces").is_some());
        assert!(r.get("total_received_bytes").is_some());
    }

    #[tokio::test]
    async fn test_processes_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("processes"));
        params.insert("process_count".to_string(), serde_json::json!(5));
        let r = tool.execute(&params).await.unwrap();
        assert!(r["total_processes"].as_u64().unwrap() > 0);
        let procs = r["top_processes"].as_array().unwrap();
        assert!(procs.len() <= 5);
        for p in procs {
            assert!(p.get("pid").is_some());
            assert!(p.get("name").is_some());
        }
    }

    #[tokio::test]
    async fn test_processes_custom_count() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("processes"));
        params.insert("process_count".to_string(), serde_json::json!(3));
        let r = tool.execute(&params).await.unwrap();
        assert!(r["top_processes"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_all_action() {
        let tool = SysMonitorTool;
        let mut params = HashMap::new();
        params.insert("action".to_string(), serde_json::json!("all"));
        let r = tool.execute(&params).await.unwrap();
        assert!(r.get("system").is_some());
        assert!(r.get("cpu").is_some());
        assert!(r.get("memory").is_some());
        assert!(r.get("disk").is_some());
        assert!(r.get("network").is_some());
        assert!(r.get("processes").is_some());
    }
}
