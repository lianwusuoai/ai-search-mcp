use std::fs;
use std::io::{self, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process;
use fs2::FileExt;

/// 守护进程管理器
pub struct DaemonManager {
    pid_file: PathBuf,
    lock_file: PathBuf,
    port: u16,
    _lock_handle: Option<fs::File>,
}

impl DaemonManager {
    pub fn new(port: u16) -> io::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "无法获取用户主目录"))?;
        
        let daemon_dir = home.join(".ai-search-mcp");
        fs::create_dir_all(&daemon_dir)?;
        
        let pid_file = daemon_dir.join("server.pid");
        let lock_file = daemon_dir.join(format!("server-{}.lock", port));
        
        Ok(Self { 
            pid_file, 
            lock_file,
            port,
            _lock_handle: None,
        })
    }
    
    /// 原子化检查并获取端口(使用文件锁防止竞态条件)
    pub fn try_acquire_port(&mut self) -> io::Result<bool> {
        // 1. 尝试获取文件锁(非阻塞)
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.lock_file)?;
        
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // 成功获取锁,继续检查
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // 锁已被占用,说明有其他实例正在运行
                tracing::info!("检测到其他实例正在运行(端口 {})", self.port);
                return Ok(false);
            }
            Err(e) => {
                return Err(e);
            }
        }
        
        // 2. 检查是否已有实例运行
        if self.check_existing_instance_internal()? {
            // 释放锁
            let _ = lock_file.unlock();
            return Ok(false);
        }
        
        // 3. 尝试绑定端口验证可用性
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))
            .map_err(|_| {
                let _ = lock_file.unlock();
                io::Error::new(io::ErrorKind::AddrInUse, 
                    format!("端口 {} 已被其他程序占用", self.port))
            })?;
        
        // 4. 立即释放端口,让 Axum 绑定
        drop(listener);
        
        // 5. 保持文件锁直到进程结束
        self._lock_handle = Some(lock_file);
        
        Ok(true)
    }
    
    /// 检查是否已有实例在运行
    fn check_existing_instance_internal(&self) -> io::Result<bool> {
        if !self.pid_file.exists() {
            return Ok(false);
        }
        
        // 读取 PID 文件
        let content = fs::read_to_string(&self.pid_file)?;
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.len() < 2 {
            // PID 文件格式错误,删除
            let _ = fs::remove_file(&self.pid_file);
            return Ok(false);
        }
        
        let pid: u32 = lines[0].parse().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "无效的 PID")
        })?;
        
        let port: u16 = lines[1].parse().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "无效的端口")
        })?;
        
        // 检查进程是否存在
        if !Self::is_process_running(pid) {
            // 进程已死,删除 PID 文件
            let _ = fs::remove_file(&self.pid_file);
            return Ok(false);
        }
        
        // 检查端口是否匹配
        if port == self.port {
            eprintln!("检测到已有实例运行在端口 {} (PID: {})", port, pid);
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// 创建 PID 文件
    pub fn create_pid_file(&self) -> io::Result<()> {
        let pid = process::id();
        let content = format!("{}\n{}\n", pid, self.port);
        
        let mut file = fs::File::create(&self.pid_file)?;
        file.write_all(content.as_bytes())?;
        
        eprintln!("守护进程已启动 (PID: {}, 端口: {})", pid, self.port);
        Ok(())
    }
    
    /// 删除 PID 文件
    pub fn remove_pid_file(&self) -> io::Result<()> {
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file)?;
            eprintln!("守护进程已停止");
        }
        Ok(())
    }
    
    /// 检查进程是否在运行
    #[cfg(windows)]
    fn is_process_running(pid: u32) -> bool {
        use std::process::Command;
        
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output();
        
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string())
        } else {
            false
        }
    }
    
    #[cfg(unix)]
    fn is_process_running(pid: u32) -> bool {
        use std::process::Command;
        
        Command::new("kill")
            .args(&["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        // 释放文件锁
        if let Some(lock_file) = &self._lock_handle {
            let _ = lock_file.unlock();
        }
        
        // 删除 PID 文件
        let _ = self.remove_pid_file();
        
        // 删除锁文件
        let _ = fs::remove_file(&self.lock_file);
    }
}
