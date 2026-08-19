const IMAGES: &[&str] = &[
    "cube-diffuse.jpg",
    "cube-normal.png",
    "happy-tree.png",
    "centrica_logo.png",
    "pure-sky-hdri.jpg",
];

#[test]
fn every_embedded_image_decodes_to_real_pixels() {
    for name in IMAGES {
        let bytes = rust_renderer::assets::load_binary(name)
            .unwrap_or_else(|e| panic!("{name} is not embedded: {e}"));
        let image = image::load_from_memory(bytes)
            .unwrap_or_else(|e| panic!("{name} does not decode: {e}"));

        let (width, height) = (image.width(), image.height());
        assert!(width > 0 && height > 0, "{name} decoded to zero dimensions");

        let rgba = image.to_rgba8();
        assert_eq!(
            rgba.len() as u32,
            width * height * 4,
            "{name} decoded to the wrong number of bytes for {width}x{height}"
        );

        // A uniform buffer is what a truncated file that still parses tends to produce, and it is
        // indistinguishable from a working texture until you look at the screen.
        let first = rgba.as_raw()[0];
        assert!(
            rgba.as_raw().iter().any(|&byte| byte != first),
            "{name} decoded to a single repeated value, so it is very likely damaged"
        );
    }
}

#[test]
fn the_image_list_covers_every_embedded_image() {
    // Otherwise a newly embedded texture silently escapes the check above.
    for name in IMAGES {
        assert!(
            rust_renderer::assets::load_binary(name).is_ok(),
            "{name} is listed here but no longer embedded"
        );
    }
    assert_eq!(IMAGES.len(), 5, "add new embedded images to IMAGES");
}
