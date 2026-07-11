#![cfg(feature = "built-in-models")]

use image::{GrayImage, ImageReader};
use std::{ffi::CString, mem::MaybeUninit, path::PathBuf, ptr};
use vmaf_head_sys::*;

const CASES: &[(&str, u32, u32)] = &[
    ("96x64", 96, 64),
    ("160x90", 160, 90),
    ("320x180", 320, 180),
];

struct VmafSession {
    context: *mut VmafContext,
    model: *mut VmafModel,
    _model_name: CString,
    _model_version: CString,
}

impl VmafSession {
    fn new() -> Self {
        let config = VmafConfiguration {
            log_level: VmafLogLevel_VMAF_LOG_LEVEL_NONE,
            n_threads: 0,
            n_subsample: 1,
            cpumask: 0,
            gpumask: 0,
        };
        let mut context = ptr::null_mut();
        let init_result = unsafe { vmaf_init(&mut context, config) };
        assert_eq!(init_result, 0, "vmaf_init failed with {init_result}");

        let model_name = CString::new("jpeg-integration").unwrap();
        let model_version = CString::new("vmaf_v0.6.1").unwrap();
        let mut model_config = VmafModelConfig {
            name: model_name.as_ptr(),
            flags: VmafModelFlags_VMAF_MODEL_FLAGS_DEFAULT as u64,
        };
        let mut model = ptr::null_mut();
        let load_result =
            unsafe { vmaf_model_load(&mut model, &mut model_config, model_version.as_ptr()) };
        if load_result != 0 {
            unsafe {
                vmaf_close(context);
            }
            panic!("vmaf_model_load failed with {load_result}");
        }

        let features_result = unsafe { vmaf_use_features_from_model(context, model) };
        if features_result != 0 {
            unsafe {
                vmaf_model_destroy(model);
                vmaf_close(context);
            }
            panic!("vmaf_use_features_from_model failed with {features_result}");
        }

        Self {
            context,
            model,
            _model_name: model_name,
            _model_version: model_version,
        }
    }
}

impl Drop for VmafSession {
    fn drop(&mut self) {
        unsafe {
            vmaf_model_destroy(self.model);
            let _ = vmaf_close(self.context);
        }
    }
}

struct OwnedPicture {
    picture: VmafPicture,
    owned: bool,
}

impl OwnedPicture {
    fn from_luma(image: &GrayImage) -> Self {
        let (width, height) = image.dimensions();
        let mut picture = MaybeUninit::<VmafPicture>::uninit();
        let alloc_result = unsafe {
            vmaf_picture_alloc(
                picture.as_mut_ptr(),
                VmafPixelFormat_VMAF_PIX_FMT_YUV400P,
                8,
                width,
                height,
            )
        };
        assert_eq!(
            alloc_result, 0,
            "vmaf_picture_alloc failed with {alloc_result}"
        );

        let picture = unsafe { picture.assume_init() };
        assert_eq!(picture.w[0], width);
        assert_eq!(picture.h[0], height);
        assert!(picture.stride[0] >= width as isize);
        assert!(!picture.data[0].is_null());

        let width = width as usize;
        let height = height as usize;
        let stride = picture.stride[0] as usize;
        let source = image.as_raw();
        let destination = picture.data[0].cast::<u8>();
        for row in 0..height {
            unsafe {
                ptr::copy_nonoverlapping(
                    source.as_ptr().add(row * width),
                    destination.add(row * stride),
                    width,
                );
            }
        }

        Self {
            picture,
            owned: true,
        }
    }

    fn as_mut_ptr(&mut self) -> *mut VmafPicture {
        &mut self.picture
    }

    fn transfer_to_vmaf(&mut self) {
        self.owned = false;
    }
}

impl Drop for OwnedPicture {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                let _ = vmaf_picture_unref(&mut self.picture);
            }
        }
    }
}

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file_name)
}

fn decode_fixture(file_name: &str) -> GrayImage {
    let path = fixture_path(file_name);
    ImageReader::open(&path)
        .unwrap_or_else(|error| panic!("failed to open {}: {error}", path.display()))
        .decode()
        .unwrap_or_else(|error| panic!("failed to decode {}: {error}", path.display()))
        .to_luma8()
}

fn score_pair(reference: &GrayImage, distorted: &GrayImage) -> f64 {
    assert_eq!(reference.dimensions(), distorted.dimensions());

    let session = VmafSession::new();
    let mut reference_picture = OwnedPicture::from_luma(reference);
    let mut distorted_picture = OwnedPicture::from_luma(distorted);
    let read_result = unsafe {
        vmaf_read_pictures(
            session.context,
            reference_picture.as_mut_ptr(),
            distorted_picture.as_mut_ptr(),
            0,
        )
    };
    if read_result == 0 {
        reference_picture.transfer_to_vmaf();
        distorted_picture.transfer_to_vmaf();
    }
    assert_eq!(
        read_result, 0,
        "vmaf_read_pictures failed with {read_result}"
    );

    let flush_result =
        unsafe { vmaf_read_pictures(session.context, ptr::null_mut(), ptr::null_mut(), 0) };
    assert_eq!(flush_result, 0, "VMAF flush failed with {flush_result}");

    let mut score = f64::NAN;
    let score_result = unsafe {
        vmaf_score_pooled(
            session.context,
            session.model,
            VmafPoolingMethod_VMAF_POOL_METHOD_MEAN,
            &mut score,
            0,
            0,
        )
    };
    assert_eq!(score_result, 0, "VMAF scoring failed with {score_result}");
    score
}

#[test]
fn scores_jpeg_pairs_at_multiple_resolutions() {
    for &(stem, width, height) in CASES {
        let reference = decode_fixture(&format!("{stem}_reference.jpg"));
        let distorted = decode_fixture(&format!("{stem}_distorted.jpg"));
        assert_eq!(reference.dimensions(), (width, height));
        assert_eq!(distorted.dimensions(), (width, height));

        let identical_score = score_pair(&reference, &reference);
        let distorted_score = score_pair(&reference, &distorted);
        eprintln!(
            "{width}x{height}: identical={identical_score:.4}, distorted={distorted_score:.4}"
        );

        assert!(identical_score.is_finite());
        assert!(distorted_score.is_finite());
        assert!((0.0..=100.0).contains(&identical_score));
        assert!((0.0..=100.0).contains(&distorted_score));
        assert!(identical_score > 90.0, "unexpected identical score");
        assert!(
            distorted_score < identical_score - 0.5,
            "distortion did not reduce the VMAF score"
        );
    }
}
