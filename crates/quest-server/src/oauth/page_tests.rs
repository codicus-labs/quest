use super::*;

#[test]
fn escapes_markup_and_quotes() {
    assert_eq!(escape("<b a=\"x\">'&"), "&lt;b a=&quot;x&quot;&gt;&#39;&amp;");
}
