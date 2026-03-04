use axum::response::sse::Event;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

// SSE 连接错误类型
#[derive(Debug)]
pub enum SseConnectionError {
    LimitExceeded,
}

impl std::fmt::Display for SseConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SseConnectionError::LimitExceeded => write!(f, "SSE connection limit exceeded"),
        }
    }
}

impl std::error::Error for SseConnectionError {}

// SSE 连接池管理

pub struct SseConnectionPool {
    current: Arc<AtomicUsize>,
    max: usize,
}

impl SseConnectionPool {
    pub fn new(max: usize) -> Self {
        Self {
            current: Arc::new(AtomicUsize::new(0)),
            max,
        }
    }
    
    pub fn try_acquire(&self) -> Result<SseConnectionGuard, SseConnectionError> {
        let current = self.current.fetch_add(1, Ordering::SeqCst);
        if current >= self.max {
            self.current.fetch_sub(1, Ordering::SeqCst);
            return Err(SseConnectionError::LimitExceeded);
        }
        Ok(SseConnectionGuard {
            pool: self.current.clone(),
        })
    }
    
    pub fn current_count(&self) -> usize {
        self.current.load(Ordering::SeqCst)
    }
}

pub struct SseConnectionGuard {
    pool: Arc<AtomicUsize>,
}

impl Drop for SseConnectionGuard {
    fn drop(&mut self) {
        self.pool.fetch_sub(1, Ordering::SeqCst);
    }
}

// SSE 事件构建器

pub struct SseEventBuilder;

impl SseEventBuilder {
    pub fn start(query: &str) -> Event {
        Event::default()
            .event("start")
            .data(json!({"query": query}).to_string())
    }
    
    pub fn result(index: usize, content: &str) -> Event {
        Event::default()
            .event("result")
            .data(json!({
                "index": index,
                "content": content
            }).to_string())
    }
    
    pub fn complete(total: usize, duration_ms: u64, model: &str, timestamp: &str) -> Event {
        Event::default()
            .event("complete")
            .data(json!({
                "total": total,
                "duration_ms": duration_ms,
                "model": model,
                "timestamp": timestamp
            }).to_string())
    }
    
    pub fn error(message: &str, code: &str) -> Event {
        Event::default()
            .event("error")
            .data(json!({
                "message": message,
                "code": code
            }).to_string())
    }
    
    pub fn ping(timestamp: &str) -> Event {
        Event::default()
            .event("ping")
            .data(json!({"timestamp": timestamp}).to_string())
    }
}

// 心跳任务

pub async fn heartbeat_task(tx: mpsc::Sender<Result<Event, std::convert::Infallible>>, interval_secs: u64) {
    let mut ticker = interval(Duration::from_secs(interval_secs));
    
    loop {
        ticker.tick().await;
        let timestamp = chrono::Utc::now().to_rfc3339();
        let event = SseEventBuilder::ping(&timestamp);
        
        if tx.send(Ok(event)).await.is_err() {
            break;
        }
    }
}
