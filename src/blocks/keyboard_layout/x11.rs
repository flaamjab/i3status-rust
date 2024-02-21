use xcb::xkb;

use super::*;

pub(super) struct X11 {
    connection: xcb::Connection,
}

impl X11 {
    pub(super) fn new() -> Self {
        todo!();
    }
}

#[async_trait]
impl Backend for X11 {
    async fn get_info(&mut self) -> Result<Info> {
        todo!();
    }

    async fn wait_for_change(&mut self) -> Result<()> {
        todo!();
    }
}
