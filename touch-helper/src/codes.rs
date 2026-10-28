//! Stable Linux input UAPI constants, named to make stream transformations visible.
pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_ABS: u16 = 3;
pub const SYN_DROPPED: u16 = 3;
pub const BTN_TOUCH: u16 = 0x14a;
pub const BTN_TOOL_FINGER: u16 = 0x145;
pub const ABS_MT_SLOT: u16 = 0x2f;
pub const ABS_MT_TOUCH_MAJOR: u16 = 0x30;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
pub const ABS_MT_TOOL_X: u16 = 0x3c;
pub const ABS_MT_TOOL_Y: u16 = 0x3d;
