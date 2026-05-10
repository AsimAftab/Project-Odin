use anyhow::Result;
use sysinfo::System;

use crate::integrations::process;
use crate::models::process::{PortInfo, ProcessInfo, ProcessStats};

pub struct ProcessService;

impl ProcessService {
    pub async fn get_listening_ports() -> Result<Vec<PortInfo>> {
        process::get_listening_ports().await
    }

    pub async fn find_process_by_port(port: u16) -> Result<Option<ProcessInfo>> {
        process::find_process_by_port(port).await
    }

    pub async fn get_all_processes() -> Result<Vec<ProcessInfo>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let processes = sys
            .processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string(),
                memory_mb: (process.memory() as f32) / 1024.0,
                cpu_percent: process.cpu_usage(),
                status: format!("{:?}", process.status()),
            })
            .collect();

        Ok(processes)
    }

    pub async fn get_process_stats() -> Result<ProcessStats> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let processes = sys.processes();
        let total = processes.len() as u32;
        let running = processes
            .iter()
            .filter(|(_, p)| format!("{:?}", p.status()).contains("Running"))
            .count() as u32;
        let sleeping = processes
            .iter()
            .filter(|(_, p)| format!("{:?}", p.status()).contains("Sleep"))
            .count() as u32;

        Ok(ProcessStats {
            total_processes: total,
            running,
            sleeping,
        })
    }

    pub async fn kill_process(pid: u32) -> Result<String> {
        process::kill_process_by_id(pid).await
    }

    pub async fn find_process_by_pid(pid: u32) -> Result<Option<ProcessInfo>> {
        let mut sys = System::new_all();
        sys.refresh_all();

        if let Some(process) = sys.process(sysinfo::Pid::from(pid as usize)) {
            Ok(Some(ProcessInfo {
                pid,
                name: process.name().to_string(),
                memory_mb: (process.memory() as f32) / 1024.0,
                cpu_percent: process.cpu_usage(),
                status: format!("{:?}", process.status()),
            }))
        } else {
            Ok(None)
        }
    }
}
