use tokio::io::unix::AsyncFd;
use xcb::{x, xkb};

use super::*;

pub(super) struct XcbXkb {
    connection: Connection,
    names: [String; 4],
    info: Info,
}

impl XcbXkb {
    pub(super) async fn new() -> Result<Self> {
        let (c, _) = Connection::connect_with_extensions(None, &[xcb::Extension::Xkb], &[])
            .expect("Xorg must be running");
        let xkb_ver = c
            .wait_for_reply(c.send_request(&xkb::UseExtension {
                wanted_major: 1,
                wanted_minor: 0,
            }))
            .await
            .expect("Xkeyboard extension must be supported");
        assert!(
            xkb_ver.supported(),
            "Xkeyboard version must be at least 1.0"
        );

        // Configure Xorg to send events when layout changes.
        c.send_request(&xkb::SelectEvents {
            device_spec: xkb::Id::UseCoreKbd as xkb::DeviceSpec,
            map: xkb::MapPart::empty(),
            affect_map: xkb::MapPart::empty(),
            affect_which: xkb::EventType::STATE_NOTIFY,
            clear: xkb::EventType::empty(),
            select_all: xkb::EventType::empty(),
            details: &[xkb::SelectEventsDetails::StateNotify {
                affect_state: xkb::StatePart::GROUP_STATE,
                state_details: xkb::StatePart::GROUP_STATE,
            }],
        });

        // Fetch group names.
        let core_kbd = xkb::Id::UseCoreKbd as xkb::DeviceSpec;
        let mut group_names = [String::new(), String::new(), String::new(), String::new()];
        let names_reply = c
            .wait_for_reply(c.send_request(&xkb::GetNames {
                device_spec: core_kbd,
                which: xkb::NameDetail::GROUP_NAMES,
            }))
            .await
            .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
        if let xkb::GetNamesReplyValueList::GroupNames(name_atoms) = &names_reply.value_list()[0] {
            debug_assert!(name_atoms.len() < 4);
            for (name_atom, group_name) in name_atoms.into_iter().zip(group_names.iter_mut()) {
                let name_reply = c
                    .wait_for_reply(c.send_request(&x::GetAtomName { atom: *name_atom }))
                    .await
                    .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
                *group_name = name_reply.name().to_utf8().to_string();
            }
        }

        // Fetch current group.
        let state = c
            .wait_for_reply(c.send_request(&xkb::GetState {
                device_spec: core_kbd,
            }))
            .await
            .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
        let current_group_name = &group_names[state.group() as usize];
        let info = Info::from_layout_variant_str(&current_group_name);

        Ok(Self {
            connection: c,
            names: group_names,
            info,
        })
    }
}

#[async_trait]
impl Backend for XcbXkb {
    async fn get_info(&mut self) -> Result<Info> {
        Ok(self.info.clone())
    }

    async fn wait_for_change(&mut self) -> Result<()> {
        let event = self
            .connection
            .wait_for_event()
            .await
            .map_err(|e| Error::new(format!("xcb_xkb: {e}")))?;
        if let xcb::Event::Xkb(xkb::Event::StateNotify(event)) = event {
            let layout = &self.names[event.group() as usize];
            self.info = Info::from_layout_variant_str(layout);
        }
        Ok(())
    }
}

struct Connection {
    inner: AsyncFd<xcb::Connection>,
}

impl Connection {
    pub fn connect_with_extensions(
        display_name: Option<&str>,
        mandatory: &[xcb::Extension],
        optional: &[xcb::Extension],
    ) -> xcb::ConnResult<(Self, i32)> {
        let (conn, screen) =
            xcb::Connection::connect_with_extensions(display_name, mandatory, optional)?;
        let conn = Connection {
            inner: AsyncFd::new(conn).unwrap(),
        };
        Ok((conn, screen))
    }

    pub async fn wait_for_event(&self) -> Result<xcb::Event, xcb::Error> {
        let conn = self.inner.get_ref();
        loop {
            match conn.poll_for_event() {
                Ok(Some(e)) => return Ok(e),
                Ok(None) => {
                    let mut g = self.inner.readable().await.unwrap();
                    g.clear_ready();
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn wait_for_reply<C: xcb::CookieWithReplyChecked>(
        &self,
        cookie: C,
    ) -> xcb::Result<C::Reply> {
        let conn = self.inner.get_ref();
        conn.flush()?;
        loop {
            match conn.poll_for_reply(&cookie) {
                Some(r) => return r,
                None => {
                    let mut g = self.inner.readable().await.unwrap();
                    g.clear_ready();
                    continue;
                }
            };
        }
    }

    pub fn send_request<T>(&self, r: &T) -> T::Cookie
    where
        T: xcb::Request,
        T::Cookie: xcb::Cookie,
    {
        let conn = self.inner.get_ref();
        conn.send_request(r)
    }
}
