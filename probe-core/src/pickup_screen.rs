//! 固定中文布局的离线拾取视觉词表，不是通用OCR。
//! 目前只接受已采样的化种匣琉璃百合提示；文字、图标或布局未知时返回未识别。
//! 坐标和阈值对应1240×558概览，模型来源与裁切范围见fixtures/pickup/README.md。
const W: usize = 1240;
const H: usize = 558;

fn ink(pixel: &[u8]) -> bool {
    // 中文提示使用亮色低色差笔画；排除大多数彩色地形，保留阴影之外的文字主体。
    pixel.iter().all(|v| *v > 185) && pixel.iter().max().unwrap() - pixel.iter().min().unwrap() < 60
}

fn text_score(rgb: &[u8], model: &[u8], x: usize, y: usize, w: usize, h: usize) -> f64 {
    let mut intersection = 0;
    let mut union = 0;
    for row in 0..h {
        for col in 0..w {
            let a = ink(&rgb[((y + row) * W + x + col) * 3..][..3]);
            let b = ink(&model[(row * w + col) * 3..][..3]);
            intersection += usize::from(a && b);
            union += usize::from(a || b);
        }
    }
    intersection as f64 / union.max(1) as f64
}

fn icon_matches(rgb: &[u8], model: &[u8], x: usize, y: usize, w: usize, h: usize) -> bool {
    let mut error = 0u32;
    for row in 0..h {
        for col in 0..w {
            for c in 0..3 {
                error += rgb[((y + row) * W + x + col) * 3 + c]
                    .abs_diff(model[(row * w + col) * 3 + c]) as u32;
            }
        }
    }
    error as usize / (w * h * 3) < 38
}

// 准备录像与运行时JPEG的抗锯齿不同，允许笔画相差一个概览像素。
// 双向检查整个字形，既要求模型笔画出现，也惩罚画面中多出的笔画，不能只认名称前缀。
fn glyph_score(rgb: &[u8], model: &[u8], x: usize, y: usize, w: usize, h: usize) -> f64 {
    let mut hits = 0;
    let mut total = 0;
    for row in 0..h {
        for col in 0..w {
            let actual = ink(&rgb[((y + row) * W + x + col) * 3..][..3]);
            let expected = ink(&model[(row * w + col) * 3..][..3]);
            for (present, in_model) in [(actual, true), (expected, false)] {
                if !present {
                    continue;
                }
                total += 1;
                let nearby = (row.saturating_sub(1)..=(row + 1).min(h - 1)).any(|r| {
                    (col.saturating_sub(1)..=(col + 1).min(w - 1)).any(|c| {
                        ink(if in_model {
                            &model[(r * w + c) * 3..][..3]
                        } else {
                            &rgb[((y + r) * W + x + c) * 3..][..3]
                        })
                    })
                });
                hits += usize::from(nearby);
            }
        }
    }
    hits as f64 / total.max(1) as f64
}

fn obtained_icon(rgb: &[u8], x: usize, y: usize) -> bool {
    // 仅比较有色花瓣的通道差，减少昼夜背景亮度变化对图标判据的影响。
    let model = include_bytes!("../assets/pickup-obtained-icon.rgb");
    let mut error = 0i32;
    let mut samples = 0;
    for row in 0..25 {
        for col in 0..24 {
            let m = &model[(row * 24 + col) * 3..][..3];
            if m.iter().max().unwrap() - m.iter().min().unwrap() <= 60 {
                continue;
            }
            let a = &rgb[((y + row) * W + x + col) * 3..][..3];
            for channel in [0, 2] {
                error +=
                    ((a[channel] as i32 - a[1] as i32) - (m[channel] as i32 - m[1] as i32)).abs();
                samples += 1;
            }
        }
    }
    samples >= 40 && error / samples < 25
}

pub(crate) fn prompt(rgb: &[u8]) -> Option<(u32, u32)> {
    if rgb.len() != W * H * 3 {
        return None;
    }
    let model = include_bytes!("../assets/pickup-seed-text.rgb");
    // 搜索实际交互行，不从世界坐标推断点击位置；输出时转换回全屏像素。
    for y in 165..375 {
        for x in 765..785 {
            if text_score(rgb, model, x, y, 148, 19) > 0.88
                && icon_matches(
                    rgb,
                    include_bytes!("../assets/pickup-seed-icon.rgb"),
                    x - 31,
                    y - 3,
                    22,
                    26,
                )
            {
                return Some(((x + 60) as u32 * 2, (y + 9) as u32 * 2));
            }
        }
    }
    None
}

pub(crate) fn obtained(rgb: &[u8]) -> bool {
    if rgb.len() != W * H * 3 {
        return false;
    }
    // 必须同时确认物品全名、独立数字1、右侧无追加数字、获得标题和花瓣图标。
    // 数字仅占整行少量笔画，单靠整行分数会误接受数量缺失或×11，故单独强制匹配。
    for y in 250..355 {
        for x in 143..156 {
            if text_score(
                rgb,
                include_bytes!("../assets/pickup-obtained-text.rgb"),
                x,
                y,
                88,
                20,
            ) > 0.45
                && glyph_score(
                    rgb,
                    include_bytes!("../assets/pickup-obtained-text.rgb"),
                    x,
                    y,
                    88,
                    20,
                ) > 0.93
                && text_score(
                    rgb,
                    include_bytes!("../assets/pickup-obtained-quantity.rgb"),
                    x + 76,
                    y,
                    12,
                    20,
                ) > 0.5
                && glyph_score(
                    rgb,
                    include_bytes!("../assets/pickup-obtained-quantity.rgb"),
                    x + 76,
                    y,
                    12,
                    20,
                ) > 0.9
                && (0..20)
                    .flat_map(|row| (88..100).map(move |col| (row, col)))
                    .filter(|(row, col)| ink(&rgb[((y + row) * W + x + col) * 3..][..3]))
                    .count()
                    < 4
                && glyph_score(
                    rgb,
                    include_bytes!("../assets/pickup-obtained-heading.rgb"),
                    x + 5,
                    y - 27,
                    32,
                    18,
                ) > 0.85
                && obtained_icon(rgb, x - 27, y - 1)
            {
                return true;
            }
        }
    }
    false
}
