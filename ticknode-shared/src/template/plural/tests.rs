use super::*;
use crate::DataType;
use crate::quantity::CanonicalLabels;

struct UkTestLabels;
impl QuantityLabels for UkTestLabels {
    fn locale(&self) -> String {
        "uk".into()
    }
    fn decimal_separator(&self) -> String {
        ",".into()
    }
    fn unit_symbol(&self, _: &str) -> Option<String> {
        None
    }
}

fn render_template(
    source: &str,
    inputs: &[(String, Option<DataValue>, DataType)],
    labels: Arc<dyn QuantityLabels + Send + Sync>,
) -> Result<String, crate::template::TemplateError> {
    let compiled = crate::template::CompiledTemplate::new(source)?;
    compiled.render_localized(
        inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
        crate::template::RenderEnv {
            labels,
            now: chrono::Utc::now(),
            ..Default::default()
        },
        &crate::TypeRegistry::new(),
    )
}

#[test]
fn test_positional_uk() {
    let labels = Arc::new(UkTestLabels);
    let template = "{{ n | plural('яблуко', 'яблука', 'яблук') }}";

    // 1 -> one -> яблуко
    let res = render_template(
        template,
        &[("n".into(), Some(DataValue::from(1)), DataType::Any)],
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res, "яблуко");

    // 3 -> few -> яблука
    let res = render_template(
        template,
        &[("n".into(), Some(DataValue::from(3)), DataType::Any)],
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res, "яблука");

    // 5 -> many -> яблук
    let res = render_template(
        template,
        &[("n".into(), Some(DataValue::from(5)), DataType::Any)],
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res, "яблук");

    // 21 -> one -> яблуко
    let res = render_template(
        template,
        &[("n".into(), Some(DataValue::from(21)), DataType::Any)],
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res, "яблуко");

    // 1.5 -> other -> falls back to last positional form -> яблук
    let res = render_template(
        template,
        &[("n".into(), Some(DataValue::from(1.5)), DataType::Any)],
        labels,
    )
    .unwrap();
    assert_eq!(res, "яблук");
}

#[test]
fn test_keyword_en() {
    let labels = Arc::new(CanonicalLabels);
    let template = "{{ n | plural(one='1 item', other='multiple items') }}";

    let res1 = render_template(
        template,
        &[("n".into(), Some(DataValue::from(1)), DataType::Any)],
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res1, "1 item");

    let res2 = render_template(
        template,
        &[("n".into(), Some(DataValue::from(2)), DataType::Any)],
        labels,
    )
    .unwrap();
    assert_eq!(res2, "multiple items");
}

#[test]
fn test_missing_category_fallback() {
    // Keyword fallback when other is missing: falls back to last keyword form
    let labels_en = Arc::new(CanonicalLabels);
    let template_kw = "{{ n | plural(one='only single') }}";
    let res = render_template(
        template_kw,
        &[("n".into(), Some(DataValue::from(5)), DataType::Any)],
        labels_en,
    )
    .unwrap();
    assert_eq!(res, "only single");

    // Positional fallback when category is missing and other is missing: falls back to last positional
    let labels_uk = Arc::new(UkTestLabels);
    let template_pos = "{{ n | plural('один', 'декілька') }}";
    let res = render_template(
        template_pos,
        &[("n".into(), Some(DataValue::from(5)), DataType::Any)],
        labels_uk,
    )
    .unwrap();
    assert_eq!(res, "декілька");
}

#[test]
fn test_hash_replacement() {
    let labels_en = Arc::new(CanonicalLabels);
    let template_en = "{{ n | plural('# file', '# files') }}";

    let res1 = render_template(
        template_en,
        &[("n".into(), Some(DataValue::from(1)), DataType::Any)],
        labels_en.clone(),
    )
    .unwrap();
    assert_eq!(res1, "1 file");

    let res2 = render_template(
        template_en,
        &[("n".into(), Some(DataValue::from(5)), DataType::Any)],
        labels_en,
    )
    .unwrap();
    assert_eq!(res2, "5 files");

    // Hash replacement with uk locale decimal separator ','
    let labels_uk = Arc::new(UkTestLabels);
    let template_uk = "{{ n | plural(one='# файл', other='# файлу') }}";
    let res_frac = render_template(
        template_uk,
        &[("n".into(), Some(DataValue::from(1.5)), DataType::Any)],
        labels_uk,
    )
    .unwrap();
    assert_eq!(res_frac, "1,5 файлу");
}

#[test]
fn test_error_on_string() {
    let labels = Arc::new(CanonicalLabels);
    let template = "{{ 'hello' | plural('a', 'b') }}";
    let res = render_template(template, &[], labels);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.message.contains("plural expects a number"));
}

#[test]
fn test_numeric_string_input() {
    let labels = Arc::new(CanonicalLabels);
    let template = "{{ '5' | plural('# apple', '# apples') }}";
    let res = render_template(template, &[], labels).unwrap();
    assert_eq!(res, "5 apples");
}

#[test]
fn test_undefined_input() {
    let labels = Arc::new(CanonicalLabels);
    let template = "{{ missing | plural('apple', 'apples') }}";
    let res =
        render_template(template, &[("missing".into(), None, DataType::Any)], labels).unwrap();
    assert_eq!(res, "");
}
