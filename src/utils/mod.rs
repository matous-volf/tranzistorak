use serenity::async_trait;

#[async_trait]
pub(crate) trait AsyncIterator {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
