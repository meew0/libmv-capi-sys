#![allow(warnings)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Forces the linker to resolve every function belonging to each enabled feature.
///
/// Rust emits no undefined symbol for an `extern` function that is merely declared,
/// so a missing libmv library (due to compilation failure or the like)
/// does not cause an error when building this crate;
/// it would show up as an unresolved symbol when a *dependent* crate
/// finally calls a required function, which is a bad place to find out.
/// Taking the address of each function makes the failure happen here instead,
/// and allows us to check each area separately with `cargo test --features <area>`.
#[cfg(test)]
mod link {
    use super::*;

    macro_rules! link_smoke {
        ($name:ident, $($function:ident),+ $(,)?) => {
            #[test]
            fn $name() {
                let addresses: &[*const ()] =
                    &[$($function as *const ()),+];
                assert!(addresses.iter().all(|address| !address.is_null()));
            }
        };
    }

    link_smoke!(
        logging,
        libmv_initLogging,
        libmv_setLoggingVerbosity,
        libmv_startDebugLogging,
    );

    #[cfg(feature = "image")]
    link_smoke!(
        image,
        libmv_floatImageDestroy,
        libmv_samplePlanarPatchByte,
        libmv_samplePlanarPatchFloat,
    );

    #[cfg(feature = "track-region")]
    link_smoke!(track_region, libmv_trackRegion);

    #[cfg(feature = "homography")]
    link_smoke!(homography, libmv_homography2DFromCorrespondencesEuc);

    #[cfg(feature = "camera-intrinsics")]
    link_smoke!(
        camera_intrinsics,
        libmv_cameraIntrinsicsApply,
        libmv_cameraIntrinsicsCopy,
        libmv_cameraIntrinsicsDestroy,
        libmv_cameraIntrinsicsDistortByte,
        libmv_cameraIntrinsicsDistortFloat,
        libmv_cameraIntrinsicsExtractOptions,
        libmv_cameraIntrinsicsInvert,
        libmv_cameraIntrinsicsNew,
        libmv_cameraIntrinsicsSetThreads,
        libmv_cameraIntrinsicsUndistortByte,
        libmv_cameraIntrinsicsUndistortFloat,
        libmv_cameraIntrinsicsUpdate,
    );

    #[cfg(feature = "tracks")]
    link_smoke!(
        tracks,
        libmv_tracksDestroy,
        libmv_tracksInsert,
        libmv_tracksNew,
    );

    #[cfg(feature = "detector")]
    link_smoke!(
        detector,
        libmv_countFeatures,
        libmv_detectFeaturesByte,
        libmv_detectFeaturesFloat,
        libmv_featuresDestroy,
        libmv_getFeature,
    );

    #[cfg(feature = "autotrack")]
    link_smoke!(
        autotrack,
        libmv_autoTrackAddMarker,
        libmv_autoTrackDestroy,
        libmv_autoTrackGetMarker,
        libmv_autoTrackMarker,
        libmv_autoTrackNew,
        libmv_autoTrackSetMarkers,
        libmv_autoTrackSetOptions,
        libmv_tracksAddMarkerN,
        libmv_tracksDestroyN,
        libmv_tracksGetMarkerN,
        libmv_tracksMaxClipN,
        libmv_tracksMaxFrameN,
        libmv_tracksMaxTrackN,
        libmv_tracksNewN,
        libmv_tracksNumMarkersN,
        libmv_tracksRemoveMarkerN,
        libmv_tracksRemoveMarkersForTrack,
        libmv_FrameAccessorDestroy,
        libmv_FrameAccessorNew,
        libmv_frameAccessorgetTransformKey,
        libmv_frameAccessorgetTransformRun,
    );

    #[cfg(feature = "reconstruction")]
    link_smoke!(
        reconstruction,
        libmv_reconstructionDestroy,
        libmv_reconstructionExtractIntrinsics,
        libmv_reconstructionIsValid,
        libmv_reprojectionCameraForImage,
        libmv_reprojectionError,
        libmv_reprojectionErrorForImage,
        libmv_reprojectionErrorForTrack,
        libmv_reprojectionPointForTrack,
        libmv_solveModal,
        libmv_solveReconstruction,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "tracks")]
    #[test]
    fn it_works() {
        let tracks = unsafe { libmv_tracksNew() };
        assert!(!tracks.is_null());
        unsafe { libmv_tracksDestroy(tracks) };
    }

    /// Returns a black image with a 5x3 white square at the given position.
    #[cfg(feature = "track-region")]
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
    #[cfg(feature = "track-region")]
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
