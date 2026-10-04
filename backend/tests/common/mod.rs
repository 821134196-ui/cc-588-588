use petcare_backend::build_app;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct TestApp {
    pub addr: String,
    pub http: reqwest::Client,
    // 保活到测试结束：临时数据库与上传目录
    _dir: TempDir,
}

impl TestApp {
    pub async fn spawn() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let upload_dir: PathBuf = dir.path().join("uploads");
        let static_dir: PathBuf = dir.path().join("static");
        let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let app = build_app(&database_url, upload_dir, static_dir)
            .await
            .expect("build app");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // 等待服务就绪
        let http = reqwest::Client::new();
        for _ in 0..30 {
            if http
                .post(format!("{addr}/api/auth/login"))
                .json(&serde_json::json!({"username":"alice","password":"staff123"}))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        TestApp {
            addr,
            http,
            _dir: dir,
        }
    }

    pub async fn login(&self, username: &str, password: &str) -> String {
        let resp = self
            .http
            .post(format!("{}/api/auth/login", self.addr))
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "login {username} 应成功");
        let v: Value = resp.json().await.unwrap();
        v["token"].as_str().unwrap().to_string()
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    /// 取一只宠物的首个指定类型 pending 任务
    pub async fn first_pending_task(
        &self,
        token: &str,
        pet_id: &str,
        kind: &str,
    ) -> Value {
        let resp = self
            .http
            .get(self.url("/api/tasks"))
            .bearer_auth(token)
            .query(&[("pet_id", pet_id), ("status", "pending"), ("kind", kind)])
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let v: Value = resp.json().await.unwrap();
        v["tasks"]
            .as_array()
            .and_then(|a| a.first().cloned())
            .expect("应至少存在一个待执行任务")
    }

    pub async fn timeline(&self, token: &str, pet_id: &str) -> Value {
        let resp = self
            .http
            .get(self.url(&format!("/api/pets/{pet_id}/timeline")))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        resp.json().await.unwrap()
    }
}
