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
pub struct Message56 {
    pub data: [u8; 56],
}

impl Default for Message56 {
    fn default() -> Self {
        Self { data: [0; 56] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message120 {
    pub data: [u8; 120],
}
impl Default for Message120 {
    fn default() -> Self {
        Self { data: [0; 120] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message248 {
    pub data: [u8; 248],
}
impl Default for Message248 {
    fn default() -> Self {
        Self { data: [0; 248] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message504 {
    pub data: [u8; 504],
}
impl Default for Message504 {
    fn default() -> Self {
        Self { data: [0; 504] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message1016 {
    pub data: [u8; 1016],
}
impl Default for Message1016 {
    fn default() -> Self {
        Self { data: [0; 1016] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message2040 {
    pub data: [u8; 2040],
}
impl Default for Message2040 {
    fn default() -> Self {
        Self { data: [0; 2040] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message4088 {
    pub data: [u8; 4088],
}
impl Default for Message4088 {
    fn default() -> Self {
        Self { data: [0; 4088] }
    }
}

#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message1656 {
    pub data: [u8; 1656],
}
impl Default for Message1656 {
    fn default() -> Self {
        Self { data: [0; 1656] }
    }
}
#[derive(Clone, Copy,Debug)]
#[repr(C)]
pub struct Message7224 {
    pub data: [u8; 7224],
}
impl Default for Message7224 {
    fn default() -> Self {
        Self { data: [0; 7224] }
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
