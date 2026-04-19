use crate::cli::capture::ansi::StyledCell;
use crate::cli::shared_html::{ansi_256_to_rgb, push_escaped};

/// Serialize a capture cell grid to a standalone HTML document wrapped
/// in a macOS-style window chrome (rounded corners, traffic-light
/// dots, title bar). The downstream Playwright screenshot targets the
/// `.mac-window` element so the chrome is baked directly into every
/// PNG/WebM frame. Downstream components don't need to re-draw it.
///
/// Per-cell rendering: one `<span>` per cell carrying inline
/// colour / bold / reversed / underlined styles. Palette and escape
/// logic come from `cli::shared_html`.
pub fn render_html(cells: &[Vec<StyledCell>]) -> String {
    let mut out = String::new();
    out.push_str(
        "<!doctype html>\n\
         <html><head><meta charset=\"utf-8\"><style>\n\
         /* HTML is deliberately bare — no body padding / no body \
            background. The decorative gradient + 32 px padding is \
            applied when the PNG is composed (scripts/render-frames.mjs). \
            This keeps the raw HTML minimal and makes the composite \
            step uniform across every scenario. */\n\
         html,body{margin:0;padding:0;background:transparent}\n\
         .mac-window{\
             /* Shrink to wrap the pre's content so cropped scenarios \
                don't leave empty dark space to the right. `max-width` \
                caps at 100% body so the window never overflows the \
                responsive viewport; `margin:0 auto` centres it. \
                \
                No box-shadow here: Playwright's element.screenshot \
                includes ink-overflow from box-shadow, which would \
                leak semi-transparent grey into the rounded corners \
                of the captured PNG. The drop-shadow is re-added \
                during the sharp compose step (scripts/render-frames.mjs) \
                where we have full control over its bounds. */\
             display:block;width:max-content;max-width:100%;\
             margin:0 auto;\
             border-radius:12px;\
             overflow:hidden;\
             background:#1a1b26;\
         }\n\
         .mac-chrome{\
             position:relative;\
             display:flex;align-items:center;\
             padding:10px 14px;\
             background:#13141f;\
             border-bottom:1px solid rgba(255,255,255,0.06);\
         }\n\
         .mac-dots{display:flex;gap:8px;z-index:1}\n\
         .mac-dot{width:12px;height:12px;border-radius:50%;display:inline-block}\n\
         .mac-dot.r{background:#ff5f57}\n\
         .mac-dot.y{background:#febc2e}\n\
         .mac-dot.g{background:#28c840}\n\
         .stage{position:relative}\n\
         pre{font-family:'JetBrains Mono',ui-monospace,monospace;\
             /* Responsive font-size: shrinks so the 140×46 pre fits \
                both dimensions of the viewport without scroll. Body \
                padding is 0 (the decorative padding is added to the \
                PNG, not the HTML), so only pre padding + chrome \
                factor in: \
                  width : 140*fs*0.6 + 32 (pre pad) = 100vw \
                          → fs = (100vw − 32) / 84 \
                  height: 46*fs*1.2 + 32 + 40 (chrome) = 100vh \
                          → fs = (100vh − 72) / 55 \
                min() picks the dimension that constrains first. \
                clamp() floors at 9px for legibility, caps at 14px. */\
             font-size:clamp(9px,min(calc((100vw - 32px) / 84),calc((100vh - 72px) / 55)),14px);\
             line-height:1.2;margin:0;padding:16px;\
             color:#e0e0e0;background:#1a1b26;white-space:pre}\n\
         span{display:inline-block}\n\
         /* Subtle blur on the right half (agent pane) so the eye \
            lands on the sidebar first. The content stays legible. */\n\
         .blur-right{\
             position:absolute;top:0;bottom:0;\
             /* Divider sits at col 46 (the sidebar's right edge), so \
                the right pane starts at col 47. Use ch units so the \
                position tracks the font-size. font-family must match \
                <pre> exactly — `ch` is the advance width of '0' in \
                the ELEMENT's own font, not the pre's. Without an \
                explicit font, `ch` would be measured against the body \
                default (sans-serif) and the blur would land several \
                columns off. 16px = pre's left padding. */\
             font-family:'JetBrains Mono',ui-monospace,monospace;\
             font-size:clamp(9px,min(calc((100vw - 32px) / 84),calc((100vh - 72px) / 55)),14px);\
             left:calc(16px + 47ch);right:0;\
             backdrop-filter:blur(1.2px);\
             -webkit-backdrop-filter:blur(1.2px);\
             pointer-events:none;\
         }\n\
         </style></head><body>\
         <div class=\"mac-window\">\
         <div class=\"mac-chrome\">\
         <div class=\"mac-dots\">\
         <span class=\"mac-dot r\"></span>\
         <span class=\"mac-dot y\"></span>\
         <span class=\"mac-dot g\"></span>\
         </div>\
         </div><div class=\"stage\"><pre>",
    );

    for row in cells {
        for cell in row {
            out.push_str("<span");
            let mut styles: Vec<String> = Vec::new();
            // ANSI reverse (SGR 7) swaps fg and bg rather than inverting
            // every colour channel — `filter:invert(1)` would give the
            // wrong hue on coloured backgrounds.
            let (fg, bg) = if cell.reversed {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };
            if let Some(n) = fg {
                styles.push(format!("color:#{:06x}", ansi_256_to_rgb(n)));
            }
            if let Some(n) = bg {
                styles.push(format!("background:#{:06x}", ansi_256_to_rgb(n)));
            }
            if cell.bold {
                styles.push("font-weight:700".into());
            }
            if cell.underlined {
                styles.push("text-decoration:underline".into());
            }
            if !styles.is_empty() {
                out.push_str(" style=\"");
                out.push_str(&styles.join(";"));
                out.push('"');
            }
            out.push('>');
            let mut tmp = [0u8; 4];
            push_escaped(&mut out, cell.ch.encode_utf8(&mut tmp));
            out.push_str("</span>");
        }
        out.push('\n');
    }

    out.push_str("</pre><div class=\"blur-right\"></div></div></div></body></html>\n");
    out
}
