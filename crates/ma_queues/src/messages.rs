#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message8 {
    pub data: [u8; 8],
}
impl Default for Message8 {
    fn default() -> Self {
        Self { data: [0; 8] }
    }
}
#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message30 {
    pub data: [u8; 30],
}
impl Default for Message30 {
    fn default() -> Self {
        Self { data: [0; 30] }
    }
}
#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message32 {
    pub data: [u8; 32],
}
impl Default for Message32 {
    fn default() -> Self {
        Self { data: [0; 32] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message60 {
    pub data: [u8; 60],
}

impl Default for Message60 {
    fn default() -> Self {
        Self { data: [0; 60] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message124 {
    pub data: [u8; 124],
}
impl Default for Message124 {
    fn default() -> Self {
        Self { data: [0; 124] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message252 {
    pub data: [u8; 252],
}
impl Default for Message252 {
    fn default() -> Self {
        Self { data: [0; 252] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message508 {
    pub data: [u8; 508],
}
impl Default for Message508 {
    fn default() -> Self {
        Self { data: [0; 508] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message1020 {
    pub data: [u8; 1020],
}
impl Default for Message1020 {
    fn default() -> Self {
        Self { data: [0; 1020] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message2044 {
    pub data: [u8; 2044],
}
impl Default for Message2044 {
    fn default() -> Self {
        Self { data: [0; 2044] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message4092 {
    pub data: [u8; 4092],
}
impl Default for Message4092 {
    fn default() -> Self {
        Self { data: [0; 4092] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message1660 {
    pub data: [u8; 1660],
}
impl Default for Message1660 {
    fn default() -> Self {
        Self { data: [0; 1660] }
    }
}
#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message7228 {
    pub data: [u8; 7228],
}
impl Default for Message7228 {
    fn default() -> Self {
        Self { data: [0; 7228] }
    }
}
#[repr(packed)]
pub struct MessageHeader {
    id: i64,                //8
    correlation_id: i64,    //16
    pub(crate) length: u16, //18
    msg_type: u16,          //20
}

pub struct PublishArg<'a> {
    pub(crate) header: &'a MessageHeader,
    pub(crate) content: *const u8,
    pub(crate) header_len: i32, // in bytes?
    pub(crate) content_len: i32,
    producer_id: i32,
}
