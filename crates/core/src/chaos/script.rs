use super::identity::ChaosIdentity;

/// Builds the `<script>` payload that makes the page's own JavaScript see
/// the same chaos identity as the network layer: `navigator.userAgent`/
/// `navigator.platform` and WebGL's vendor/renderer strings all get
/// overridden to match, so nothing about the identity is internally
/// inconsistent (a UA that disagrees with `navigator.userAgent` is itself a
/// distinguishing signal — the opposite of the goal here).
pub fn build_injection_script(identity: &ChaosIdentity, user_agent: &str) -> String {
    format!(
        r#"<script>(function(){{
try {{
  var ua = {ua};
  var platform = {platform};
  var vendor = {vendor};
  var renderer = {renderer};

  var define = function(obj, prop, value) {{
    try {{
      Object.defineProperty(obj, prop, {{ get: function () {{ return value; }}, configurable: true }});
    }} catch (e) {{}}
  }};

  define(Navigator.prototype, 'userAgent', ua);
  define(Navigator.prototype, 'platform', platform);
  define(window.navigator, 'userAgent', ua);
  define(window.navigator, 'platform', platform);

  var spoofGl = function (proto) {{
    if (!proto || !proto.getParameter) return;
    var orig = proto.getParameter;
    proto.getParameter = function (param) {{
      if (param === 37445 || param === 7936) return vendor;   // UNMASKED_VENDOR_WEBGL / VENDOR
      if (param === 37446 || param === 7937) return renderer;  // UNMASKED_RENDERER_WEBGL / RENDERER
      return orig.apply(this, arguments);
    }};
  }};
  try {{ spoofGl(window.WebGLRenderingContext && window.WebGLRenderingContext.prototype); }} catch (e) {{}}
  try {{ spoofGl(window.WebGL2RenderingContext && window.WebGL2RenderingContext.prototype); }} catch (e) {{}}
}} catch (e) {{}}
}})();</script>"#,
        ua = js_string(user_agent),
        platform = js_string(identity.os),
        vendor = js_string(identity.hardware),
        renderer = js_string(identity.hardware),
    )
}

fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '<' => out.push_str("\\u003C"),
            '>' => out.push_str("\\u003E"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chaos::identity::ChaosIdentity;

    #[test]
    fn script_embeds_identity_and_stays_closed() {
        let identity = ChaosIdentity::for_domain(b"seed", "example.com");
        let ua = identity.user_agent();
        let script = build_injection_script(&identity, &ua);

        assert!(script.starts_with("<script>"));
        assert!(script.trim_end().ends_with("</script>"));
        assert!(script.contains(identity.hardware));
        assert!(script.contains(identity.os));
    }

    #[test]
    fn js_string_escapes_forbidden_sequences() {
        assert_eq!(js_string("a\"b</script>c"), "\"a\\\"b\\u003C/script\\u003Ec\"");
    }
}
