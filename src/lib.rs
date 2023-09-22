#![allow(warnings)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let tracks = unsafe { libmv_tracksNew() };
        assert!(!tracks.is_null());
        unsafe { libmv_tracksDestroy(tracks) };
    }
}
