use xcb::{x, xkb};

use super::*;

pub(super) struct XcbXkb {
    connection: xcb::Connection,
    update_interval: Seconds,
}

impl XcbXkb {
    pub(super) fn new(update_interval: Seconds) -> Self {
        let (connection, _) =
            xcb::Connection::connect_with_extensions(None, &[xcb::Extension::Xkb], &[])
                .expect("Xorg must be running");
        let xkb_ver = connection
            .wait_for_reply(connection.send_request(&xkb::UseExtension {
                wanted_major: 1,
                wanted_minor: 0,
            }))
            .expect("Xkeyboard extension must be supported");
        assert!(
            xkb_ver.supported(),
            "Xkeyboard version must be at least 1.0"
        );
        Self {
            connection,
            update_interval,
        }
    }
}

#[async_trait]
impl Backend for XcbXkb {
    async fn get_info(&mut self) -> Result<Info> {
        let core_kbd = xkb::Id::UseCoreKbd as xkb::DeviceSpec;
        let state = self
            .connection
            .wait_for_reply(self.connection.send_request(&xkb::GetState {
                device_spec: core_kbd,
            }))
            .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
        let names = self
            .connection
            .wait_for_reply(self.connection.send_request(&xkb::GetNames {
                device_spec: core_kbd,
                which: xkb::NameDetail::GROUP_NAMES,
            }))
            .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
        if let xkb::GetNamesReplyValueList::GroupNames(group_names) = &names.value_list()[0] {
            let name_reply = self
                .connection
                .wait_for_reply(self.connection.send_request(&x::GetAtomName {
                    atom: group_names[state.group() as usize],
                }))
                .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
            let name = name_reply.name().to_utf8();
            return Ok(Info::from_layout_variant_str(&name));
        }
        unreachable!();
    }

    async fn wait_for_change(&mut self) -> Result<()> {
        sleep(self.update_interval.0).await;
        Ok(())
    }
}
