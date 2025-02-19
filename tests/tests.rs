use pdf_writer::{
    Date, Filter, Finish, Name, Null, Obj, PdfWriter, Rect, Ref, Str, TextStr,
};

/// Test that `buf` is the same as the result of concatenating the strings.
macro_rules! test {
    ($buf:expr, $($expected:literal),* $(,)?) => {{
        let buf = $buf;
        let mut expected = vec![];
        $(expected.extend($expected);)*
        if buf != expected {
            println!("=========== EXPECTED =============");
            println!("{}", String::from_utf8_lossy(&expected));
            println!("============= FOUND ==============");
            println!("{}", String::from_utf8_lossy(&buf));
            println!("=============================");
            panic!("assertion failed");
        }
    }}
}

/// Test how an object is written.
macro_rules! test_obj {
    (|$obj:ident| $write:expr, $($tts:tt)*) => {{
        test!(slice_obj(|$obj| { $write; }), $($tts)*)
    }}
}

/// Test how a primitive object is written.
macro_rules! test_primitive {
    ($value:expr, $($tts:tt)*) => {
        test_obj!(|obj| obj.primitive($value), $($tts)*);
    }
}

/// Return the slice of bytes written during the execution of `f`.
fn slice<F>(f: F) -> Vec<u8>
where
    F: FnOnce(&mut PdfWriter),
{
    let mut w = PdfWriter::new();
    let start = w.len();
    f(&mut w);
    let end = w.len();
    let buf = w.finish();
    buf[start .. end].to_vec()
}

/// Return the slice of bytes written for an object.
fn slice_obj<F>(f: F) -> Vec<u8>
where
    F: FnOnce(Obj<'_>),
{
    let buf = slice(|w| f(w.indirect(Ref::new(1))));
    buf[8 .. buf.len() - 9].to_vec()
}

#[test]
fn test_minimal() {
    let w = PdfWriter::new();
    test!(
        w.finish(),
        b"%PDF-1.7\n%\x80\x80\x80\x80\n\n",
        b"xref\n0 1\n0000000000 65535 f\r\n",
        b"trailer\n<<\n  /Size 1\n>>\n",
        b"startxref\n16\n%%EOF",
    );
}

#[test]
fn test_xref_free_list_short() {
    let mut w = PdfWriter::new();
    w.indirect(Ref::new(1)).primitive(1);
    w.indirect(Ref::new(2)).primitive(2);
    test!(
        w.finish(),
        b"%PDF-1.7\n%\x80\x80\x80\x80\n\n",
        b"1 0 obj\n1\nendobj\n\n",
        b"2 0 obj\n2\nendobj\n\n",
        b"xref\n",
        b"0 3\n",
        b"0000000000 65535 f\r\n",
        b"0000000016 00000 n\r\n",
        b"0000000034 00000 n\r\n",
        b"trailer\n",
        b"<<\n  /Size 3\n>>\n",
        b"startxref\n52\n%%EOF",
    )
}

#[test]
fn test_xref_free_list_long() {
    let mut w = PdfWriter::new();
    w.set_version(1, 4);
    w.indirect(Ref::new(1)).primitive(1);
    w.indirect(Ref::new(2)).primitive(2);
    w.indirect(Ref::new(5)).primitive(5);
    test!(
        w.finish(),
        b"%PDF-1.4\n%\x80\x80\x80\x80\n\n",
        b"1 0 obj\n1\nendobj\n\n",
        b"2 0 obj\n2\nendobj\n\n",
        b"5 0 obj\n5\nendobj\n\n",
        b"xref\n",
        b"0 6\n",
        b"0000000003 65535 f\r\n",
        b"0000000016 00000 n\r\n",
        b"0000000034 00000 n\r\n",
        b"0000000004 00000 f\r\n",
        b"0000000000 00000 f\r\n",
        b"0000000052 00000 n\r\n",
        b"trailer\n",
        b"<<\n  /Size 6\n>>\n",
        b"startxref\n70\n%%EOF",
    )
}

#[test]
#[should_panic(expected = "duplicate indirect reference id: 3")]
fn test_xref_free_list_duplicate() {
    let mut w = PdfWriter::new();
    w.indirect(Ref::new(3)).primitive(1);
    w.indirect(Ref::new(5)).primitive(2);
    w.indirect(Ref::new(13)).primitive(1);
    w.indirect(Ref::new(3)).primitive(1);
    w.indirect(Ref::new(6)).primitive(2);
    w.finish();
}

#[test]
fn test_primitive_objects() {
    // Test really simple objects.
    test_primitive!(true, b"true");
    test_primitive!(false, b"false");
    test_primitive!(78, b"78");
    test_primitive!(4.22, b"4.22");
    test_primitive!(1.184e-7, b"0.0000001184");
    test_primitive!(4.2e13, b"42000000000000");
    test_primitive!(Ref::new(7), b"7 0 R");
    test_primitive!(Null, b"null");

    // Test strings.
    test_primitive!(Str(b"Hello, World!"), b"(Hello, World!)");
    test_primitive!(Str(b"()"), b"<2829>");
    test_primitive!(Str(br"\"), b"<5C>");
    test_primitive!(Str(br"\n"), b"<5C6E>");

    // Test text strings.
    test_primitive!(TextStr("Hallo"), b"(Hallo)");
    test_primitive!(TextStr("😀!"), b"(\xFE\xFF\xD8\x3D\xDE\0\0!)");

    // Test names.
    test_primitive!(Name(b"Filter"), b"/Filter");
    test_primitive!(Name(b"A B"), br"/A#20B");
    test_primitive!(Name(b"~+c"), br"/#7E#2Bc");
    test_primitive!(Name(b"#"), br"/#23");
    test_primitive!(Name(b"\n"), br"/#0A");
}

#[test]
fn test_dates() {
    test_primitive!(Date::new(2021), b"(D:2021)");
    test_primitive!(Date::new(2021).month(30), b"(D:202112)");

    let date = Date::new(2020).month(3).day(17).hour(1).minute(2).second(3);
    test_primitive!(date, b"(D:20200317010203)");
    test_primitive!(date.utc_offset_hour(0), b"(D:20200317010203Z)");
    test_primitive!(date.utc_offset_hour(4), b"(D:20200317010203+04'00)");
    test_primitive!(
        date.utc_offset_hour(-17).utc_offset_minute(10),
        b"(D:20200317010203-17'10)"
    );
}

#[test]
fn test_arrays() {
    test_obj!(|obj| obj.array(), b"[]");
    test_obj!(|obj| obj.array().item(12).item(Null), b"[12 null]");
    test_obj!(|obj| obj.array().typed().items(vec![1, 2, 3]), b"[1 2 3]");
    test_obj!(
        |obj| {
            let mut array = obj.array();
            array.push().array().typed().items(vec![1, 2]);
            array.item(3);
        },
        b"[[1 2] 3]",
    );
}

#[test]
fn test_dicts() {
    test_obj!(|obj| obj.dict(), b"<<>>");
    test_obj!(
        |obj| obj.dict().pair(Name(b"Quality"), Name(b"Good")),
        b"<<\n  /Quality /Good\n>>",
    );
    test_obj!(
        |obj| {
            obj.dict().pair(Name(b"A"), 1).pair(Name(b"B"), 2);
        },
        b"<<\n  /A 1\n  /B 2\n>>",
    );
}

#[test]
fn test_annotations() {
    test!(
        slice(|w| {
            let mut page = w.page(Ref::new(1));
            let mut annots = page.annotations();
            annots.push().rect(Rect::new(0.0, 0.0, 1.0, 1.0));
            annots.push().rect(Rect::new(1.0, 1.0, 0.0, 0.0));
            annots.finish();
            page.bleed_box(Rect::new(-100.0, -100.0, 100.0, 100.0));
        }),
        b"1 0 obj\n",
        b"<<\n",
        b"  /Type /Page\n",
        b"  /Annots [<<\n",
        b"    /Type /Annot\n",
        b"    /Rect [0 0 1 1]\n",
        b"  >> <<\n",
        b"    /Type /Annot\n",
        b"    /Rect [1 1 0 0]\n",
        b"  >>]\n",
        b"  /BleedBox [-100 -100 100 100]\n",
        b">>\n",
        b"endobj\n\n",
    );
}

#[test]
fn test_streams() {
    let mut w = PdfWriter::new();
    w.stream(Ref::new(1), &b"Hi there!"[..]).filter(Filter::Crypt);
    test!(
        w.finish(),
        b"%PDF-1.7\n%\x80\x80\x80\x80\n\n",
        b"1 0 obj\n",
        b"<<\n  /Length 9\n  /Filter /Crypt\n>>\n",
        b"stream\n",
        b"Hi there!\n",
        b"endstream\n",
        b"endobj\n\n",
        b"xref\n",
        b"0 2\n",
        b"0000000000 65535 f\r\n",
        b"0000000016 00000 n\r\n",
        b"trailer\n",
        b"<<\n  /Size 2\n>>\n",
        b"startxref\n94\n%%EOF",
    )
}
