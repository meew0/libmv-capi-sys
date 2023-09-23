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

    /// Returns a black image with a 5x3 white square at the given position.
    fn test_image_data(x: usize, y: usize) -> Vec<f32> {
        let mut data = vec![0.0; 300 * 200];

        for i in 0..15 {
            let current_x = x + i % 5;
            let current_y = y + i / 5;
            data[current_y * 300 + current_x] = 1.0;
        }

        data
    }

    /// A test that uses functions requiring Ceres,
    /// to make sure we successfully link to it.
    #[test]
    fn track_region() {
        let image1_data = test_image_data(100, 100);
        let image2_data = test_image_data(102, 103);

        let libmv_options = libmv_TrackRegionOptions {
            direction: libmv_TrackRegionDirection_LIBMV_TRACK_REGION_FORWARD,
            motion_model: 0, // translation
            num_iterations: 50,
            use_brute: 1,
            use_normalization: 0,
            minimum_correlation: 0.75,
            sigma: 0.9,
            image1_mask: std::ptr::null_mut(),
        };

        let x1: [f64; 5] = [90.0, 110.0, 110.0, 90.0, 100.0];
        let y1: [f64; 5] = [90.0, 90.0, 110.0, 110.0, 100.0];
        let mut x2 = x1.clone();
        let mut y2 = y1.clone();

        let result = unsafe {
            libmv_trackRegion(
                &libmv_options as *const libmv_TrackRegionOptions,
                image1_data.as_ptr(),
                300,
                200,
                image2_data.as_ptr(),
                300,
                200,
                x1.as_ptr(),
                y1.as_ptr(),
                std::ptr::null_mut(), // argument is not used by the C API
                x2.as_mut_ptr(),
                y2.as_mut_ptr(),
            )
        };

        assert_eq!(result, 1, "tracking should have succeeded");

        // verify that tracking did take place
        assert!((x2[4] - 102.0).abs() < 0.5);
        assert!((y2[4] - 103.0).abs() < 0.5);
    }
}
