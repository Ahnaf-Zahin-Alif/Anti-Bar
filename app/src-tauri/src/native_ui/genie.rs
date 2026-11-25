use std::sync::atomic::{AtomicBool, Ordering};

static GENIE_ENABLED: AtomicBool = AtomicBool::new(true);

/// HLSL vertex + 3-tap horizontal filter pixel shader for GPU Genie minimize/restore suction.
pub const GENIE_HLSL: &str = r#"
cbuffer GenieParams : register(b0) {
    float4 quad_bounds; // l, t, r, b
    float4 dock_target; // target_x, target_y, progress (0..1), direction (1=min,-1=restore)
    float2 surf_size;
    float2 inv_tex_size;
};

Texture2D win_tex : register(t0);
SamplerState lin_samp : register(s0);

struct VSOut {
    float4 pos : SV_Position;
    float2 uv  : TEXCOORD0;
};

VSOut vs_main(uint vid : SV_VertexID) {
    float2 corners[4] = { float2(0,0), float2(1,0), float2(0,1), float2(1,1) };
    float2 c = corners[vid];
    float px = lerp(quad_bounds.x, quad_bounds.z, c.x);
    float py = lerp(quad_bounds.y, quad_bounds.w, c.y);
    VSOut o;
    o.pos = float4((px / surf_size.x) * 2.0 - 1.0, 1.0 - (py / surf_size.y) * 2.0, 0.0, 1.0);
    o.uv = c;
    return o;
}

float4 ps_main(VSOut i) : SV_Target {
    float t = saturate(dock_target.z);
    float curve = t * t * (3.0 - 2.0 * t);
    float squeeze = lerp(1.0, 0.14, curve * i.uv.y);
    float u = (i.uv.x - 0.5) / max(squeeze, 0.05) + 0.5;
    if (u < 0.0 || u > 1.0) discard;
    float dx = inv_tex_size.x * 0.75;
    float4 c0 = win_tex.Sample(lin_samp, float2(u - dx, i.uv.y));
    float4 c1 = win_tex.Sample(lin_samp, float2(u, i.uv.y));
    float4 c2 = win_tex.Sample(lin_samp, float2(u + dx, i.uv.y));
    return (c0 * 0.25 + c1 * 0.50 + c2 * 0.25) * (1.0 - t * 0.15);
}
"#;

pub fn set_genie_enabled(enabled: bool) {
    GENIE_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_genie_enabled() -> bool {
    GENIE_ENABLED.load(Ordering::Relaxed)
}

/// Evaluates normalized horizontal suction width factor at vertical fraction y in [0, 1] and progress t in [0, 1].
pub fn genie_squeeze_width(y_norm: f32, progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0);
    let y = y_norm.clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - (1.0 - 0.14) * smooth * y
}

#[cfg(windows)]
pub fn verify_d3d11_shader_compile() -> Result<usize, String> {
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::ID3DBlob;

    unsafe {
        let mut vs_blob: Option<ID3DBlob> = None;
        let mut err_blob: Option<ID3DBlob> = None;
        D3DCompile(
            GENIE_HLSL.as_ptr() as *const _,
            GENIE_HLSL.len(),
            None,
            None,
            None,
            windows::core::s!("vs_main"),
            windows::core::s!("vs_5_0"),
            0,
            0,
            &mut vs_blob,
            Some(&mut err_blob),
        )
        .map_err(|e| format!("VS compile failed: {}", e))?;

        let mut ps_blob: Option<ID3DBlob> = None;
        D3DCompile(
            GENIE_HLSL.as_ptr() as *const _,
            GENIE_HLSL.len(),
            None,
            None,
            None,
            windows::core::s!("ps_main"),
            windows::core::s!("ps_5_0"),
            0,
            0,
            &mut ps_blob,
            Some(&mut err_blob),
        )
        .map_err(|e| format!("PS compile failed: {}", e))?;

        Ok(vs_blob.map(|b| b.GetBufferSize()).unwrap_or(0)
            + ps_blob.map(|b| b.GetBufferSize()).unwrap_or(0))
    }
}
