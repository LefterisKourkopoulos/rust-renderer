use image::GenericImageView;

#[test]
fn decodes_all_embedded_image_assets() {
    for name in [
        "cube-diffuse.jpg",
        "cube-normal.png",
        "happy-tree.png",
        "centrica_logo.png",
    ] {
        let bytes = rust_renderer::assets::load_binary(name).unwrap();
        let img = image::load_from_memory(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (w, h) = img.dimensions();
        let rgba = img.to_rgba8();
        assert!(w > 0 && h > 0, "{name} has zero dimensions");
        assert_eq!(rgba.len() as u32, w * h * 4, "{name} rgba size mismatch");
        // A decoder that silently produced a blank buffer would still pass the
        // size checks, so require actual variation in the pixel data.
        let first = rgba.as_raw()[0];
        assert!(
            rgba.as_raw().iter().any(|&b| b != first),
            "{name} decoded to a uniform buffer"
        );
        let n = (w * h) as u64;
        let mut sum = [0u64; 4];
        for px in rgba.as_raw().chunks_exact(4) {
            for c in 0..4 {
                sum[c] += px[c] as u64;
            }
        }
        println!(
            "{name}: {w}x{h} mean rgba = {} {} {} {}",
            sum[0] / n,
            sum[1] / n,
            sum[2] / n,
            sum[3] / n
        );
    }
}
