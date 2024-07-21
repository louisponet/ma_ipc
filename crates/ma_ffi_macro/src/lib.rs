use proc_macro::TokenStream;
use proc_macro2::{TokenStream as TokenStream2, Literal};
use quote::{format_ident, quote};

#[proc_macro]
pub fn ffi_msg(input: TokenStream) -> TokenStream {
    let a: usize = input.to_string().parse().unwrap();

    let mut func_stream = TokenStream2::default();

    let msgname = Literal::usize_suffixed(a);

    let fname = format_ident!("init_producer{}", a);
    func_stream.extend(quote! {

    #[no_mangle]
    #[inline(always)]
    pub extern "C" fn #fname(
        queue_ptr: *mut QueueHeader,
        producer_ptr: *mut Producer<'static, [u8; #msgname]>,
    ) -> FFIError {
        let q = match Queue::from_initialized_ptr(queue_ptr) {
            Ok(q) => q,
            Err(e) => return e.into(),
        };
        unsafe {
            (*producer_ptr).produced_first = 0;
            (*producer_ptr).queue = q;
         }
        FFIError::Success
    }});

    let fname = format_ident!("init_consumer{}", a);
    func_stream.extend(quote! {

    #[no_mangle]
    #[inline(always)]
    pub extern "C" fn #fname(
        path: *const std::os::raw::c_char,
        consumer_ptr: *mut Consumer<'static,[u8; #a]>,
    ) -> FFIError {
        let p = unsafe{ std::ffi::CStr::from_ptr(path)}.to_str().unwrap();
        let q = Queue::<[u8; #a]>::open_shared(p).unwrap();
        Consumer::init_header(consumer_ptr, q);
        FFIError::Success
    }});

    let fname = format_ident!("produce_{}", a);
    func_stream.extend(quote! {

    #[no_mangle]
    #[inline(always)]
    pub extern "C" fn #fname(
        producer: *mut Producer<'static, [u8; #a]>,
        m: &[u8; #a],
    ) -> FFIError {
        unsafe { &mut (*producer) }.produce(m);
        FFIError::Success
    }});

    let fname = format_ident!("consume_{}", a);
    func_stream.extend(quote! {

    #[no_mangle]
    #[inline(always)]
    pub extern "C" fn #fname(
        reader: *mut Consumer<'static, [u8; #a]>,
        dest: &mut [u8; #a],
    ) -> FFIError {
        loop {
            match unsafe { &mut (*reader) }.try_consume(dest) {
                Ok(()) => return FFIError::Success,
                Err(e) => return e.into(),
            }
        }
    }});

    // let fname = format_ident!("ConsumeWithCallback{}", a);
    // func_stream.extend(quote! {

    // #[no_mangle]
    // #[inline(always)]
    // pub extern "C" fn #fname(
    //     consumer_ptr: *mut Consumer<#msgname>,
    //     cb: extern "C" fn(*const #msgname, u32, FFIError) -> bool,
    // ) {
    //     // TODO: Error handling
    //     loop {
    //         match unsafe { (*consumer_ptr).try_consume() } {
    //             Ok(msg) => {
    //                 if !cb(&msg, #a, FFIError::Success) {
    //                     break;
    //                 }
    //             }
    //             Err(ReadError::SpedPast) => {
    //                 cb(& #msgname::default(), #a, FFIError::SpedPast);
    //                 break;
    //             }
    //             Err(ReadError::Empty) => continue,
    //         }
    //     }
    // }});
    
    let fname = format_ident!("Write{}", a);
    func_stream.extend(quote! {
        #[no_mangle]
        #[inline(always)]
        pub extern "C" fn #fname(
            vector: *mut u8,
            pos: u32,
            m: &[u8; #a],
        ) -> FFIError {
             let v = unsafe { &mut (*(std::ptr::slice_from_raw_parts_mut(vector, 128) as *mut SeqlockVector<[u8; #a]>))};
            v.write(pos as usize,m);
            FFIError::Success
        }
    });

    let fname = format_ident!("Read{}", a);
    func_stream.extend(quote!{
         #[no_mangle]
         #[inline(always)]
         pub extern "C" fn #fname(
             vector: *mut u8,
             pos:u32,
             dest: &mut [u8; #a],
         ) -> FFIError {
             let v = unsafe { &mut (*(std::ptr::slice_from_raw_parts_mut(vector, 128) as *mut SeqlockVector<[u8; #a]>))};
             v.read(pos as usize, dest);
             FFIError::Success
         }
     });

    func_stream.into()
}
