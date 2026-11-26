use crate::drivers::cdp::CdpSession;
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

#[async_trait]
pub trait BrowserBackend: Send + Sync {
    async fn navigate(&self, url: &str) -> Result<(), String>;
    async fn reload(&self) -> Result<(), String>;
    async fn go_back(&self) -> Result<(), String>;
    async fn go_forward(&self) -> Result<(), String>;
    async fn get_url(&self) -> Result<String, String>;
    async fn click_at_index(&self, selector: &str, index: usize) -> Result<(), String>;
    async fn hover_at_index(&self, selector: &str, index: usize) -> Result<(), String>;
    async fn type_text_at_index(
        &self,
        selector: &str,
        index: usize,
        text: &str,
        clear: bool,
        direct: bool,
    ) -> Result<(), String>;
    async fn press_key(&self, key: &str) -> Result<(), String>;
    async fn scroll(&self, dx: f64, dy: f64) -> Result<(), String>;
    async fn scroll_element(
        &self,
        selector: &str,
        index: usize,
        dx: f64,
        dy: f64,
        into_view: bool,
        smooth: bool,
    ) -> Result<bool, String>;
    async fn get_text(
        &self,
        selector: &str,
        index: Option<usize>,
        multiple: bool,
        include_tags: bool,
    ) -> Result<Value, String>;
    async fn get_attribute(
        &self,
        selector: &str,
        attr: &str,
        index: Option<usize>,
        multiple: bool,
    ) -> Result<Value, String>;
    async fn element_exists(&self, selector: &str, timeout: Duration) -> Result<bool, String>;
    async fn count_elements(&self, selector: &str) -> Result<usize, String>;
    async fn manipulate_form_element(
        &self,
        selector: &str,
        index: usize,
        form_type: &str,
        value: &str,
        clear: bool,
        selected: Option<bool>,
    ) -> Result<Value, String>;
    async fn capture_screenshot(&self, format: &str) -> Result<String, String>;
    async fn get_cookies(&self, urls: Option<Vec<String>>) -> Result<Value, String>;
    async fn set_cookie(&self, cookie: Value) -> Result<bool, String>;
    async fn delete_cookies(
        &self,
        name: &str,
        url: Option<&str>,
        domain: Option<&str>,
    ) -> Result<(), String>;
    async fn evaluate(&self, script: &str) -> Result<Value, String>;
    async fn solve_turnstile(&self, timeout: Duration) -> Result<bool, String>;
}

#[async_trait]
impl BrowserBackend for CdpSession {
    async fn navigate(&self, url: &str) -> Result<(), String> {
        self.navigate(url).await
    }
    async fn reload(&self) -> Result<(), String> {
        self.reload().await
    }
    async fn go_back(&self) -> Result<(), String> {
        self.go_back().await
    }
    async fn go_forward(&self) -> Result<(), String> {
        self.go_forward().await
    }
    async fn get_url(&self) -> Result<String, String> {
        self.get_url().await
    }
    async fn click_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
        self.click_at_index(selector, index).await
    }
    async fn hover_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
        self.hover_at_index(selector, index).await
    }
    async fn type_text_at_index(
        &self,
        selector: &str,
        index: usize,
        text: &str,
        clear: bool,
        direct: bool,
    ) -> Result<(), String> {
        self.type_text_at_index(selector, index, text, clear, direct)
            .await
    }
    async fn press_key(&self, key: &str) -> Result<(), String> {
        self.press_key(key).await
    }
    async fn scroll(&self, dx: f64, dy: f64) -> Result<(), String> {
        self.scroll(dx, dy).await
    }
    async fn scroll_element(
        &self,
        selector: &str,
        index: usize,
        dx: f64,
        dy: f64,
        into_view: bool,
        smooth: bool,
    ) -> Result<bool, String> {
        self.element_engine()
            .scroll_element(selector, index, dx, dy, into_view, smooth)
            .await
    }
    async fn get_text(
        &self,
        selector: &str,
        index: Option<usize>,
        multiple: bool,
        include_tags: bool,
    ) -> Result<Value, String> {
        self.element_engine()
            .get_text(selector, index, multiple, include_tags)
            .await
    }
    async fn get_attribute(
        &self,
        selector: &str,
        attr: &str,
        index: Option<usize>,
        multiple: bool,
    ) -> Result<Value, String> {
        self.element_engine()
            .get_attribute(selector, attr, index, multiple)
            .await
    }
    async fn element_exists(&self, selector: &str, timeout: Duration) -> Result<bool, String> {
        self.element_engine()
            .element_exists(selector, timeout)
            .await
    }
    async fn count_elements(&self, selector: &str) -> Result<usize, String> {
        self.element_engine().count_elements(selector).await
    }
    async fn manipulate_form_element(
        &self,
        selector: &str,
        index: usize,
        form_type: &str,
        value: &str,
        clear: bool,
        selected: Option<bool>,
    ) -> Result<Value, String> {
        self.element_engine()
            .manipulate_form_element(selector, index, form_type, value, clear, selected)
            .await
    }
    async fn capture_screenshot(&self, format: &str) -> Result<String, String> {
        self.capture_screenshot(format).await
    }
    async fn get_cookies(&self, urls: Option<Vec<String>>) -> Result<Value, String> {
        self.get_cookies(urls).await
    }
    async fn set_cookie(&self, cookie: Value) -> Result<bool, String> {
        self.set_cookie(cookie).await
    }
    async fn delete_cookies(
        &self,
        name: &str,
        url: Option<&str>,
        domain: Option<&str>,
    ) -> Result<(), String> {
        self.delete_cookies(name, url, domain).await
    }
    async fn evaluate(&self, script: &str) -> Result<Value, String> {
        self.evaluate(script).await
    }
    async fn solve_turnstile(&self, timeout: Duration) -> Result<bool, String> {
        self.solve_turnstile(timeout).await
    }
}
