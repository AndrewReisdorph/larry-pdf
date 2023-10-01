
use log::debug;
use nom::{
    branch::alt,
    bytes::complete::{take, tag, take_till1, take_until, is_not, escaped},
    character::complete::{char, digit1, line_ending, multispace0, multispace1, u64, alpha1, newline, alphanumeric1, none_of},
    combinator::{eof, recognize, map_res, map, verify},
    multi::{many0, many_till, count, many1, fold_many0, fold_many1},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
    IResult, Parser, number::complete::double,
};

#[derive(Debug)]
pub enum ContentToken {
    Cm(Vec<f64>),
    BeginMarkedContent(String),
    EndMarkedContent,
    StrokingColorSpaceGrey(f64),
    ColorSpaceGrey(f64),
    LineWidth(f64),
    Move((f64, f64)),
    Line((f64, f64)),
    StrokePath,
    BeginMarkedContentWithProperties,
    BeginTextObject,
    EndTextObject,
    SetTextMatrix(Vec<f64>), // Tm
    TextFont((String, f64)),
    ShowTextString(String),
    SetFlatnessTolerance(f64),
    EndPath,
    FillPathEvenOdd,
    SaveGraphicsState,
    RestoreGraphicsState,
    PaintXObject(String)
}

fn parse_tag(start_inp: &[u8]) -> IResult<&[u8], &[u8]> {
    let (inp, value) = 
        delimited(
            multispace0,
            preceded(char('/'), alphanumeric1),
            multispace1
        )(start_inp)?;

    Ok((inp, value))
}

fn parse_string(start_inp: &[u8]) -> IResult<&[u8], &[u8]> {
    let esc = escaped(none_of("\\)("), '\\', alt((tag(")"), tag("("))));
    let esc_or_empty = alt((esc, tag("")));
    let res = delimited(tag("("), esc_or_empty, tag(")"))(start_inp)?;

    Ok(res)
}

fn parse_dictionary(start_inp: &[u8]) -> IResult<&[u8], Vec<(&[u8], u64)>> {
    let (inp, value) = delimited(
        tag("<<"),
        many1(
            delimited(
                multispace0,
                separated_pair(
                    parse_tag,
                    multispace0,
                    u64
                ),
                multispace0
            )
        ),
        tag(">>")
    )(start_inp)?;

    Ok((inp, value))
}

fn parse_bdc(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(separated_pair(separated_pair(
        parse_tag, 
        multispace0,
        parse_dictionary
    ), multispace0, tag("BDC")), |value| ContentToken::BeginMarkedContentWithProperties)(start_inp)?;

    Ok((inp, value))
}

fn parse_stroke_path(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(delimited(multispace0, char('S'), multispace1), |value| ContentToken::StrokePath)(start_inp)?;

    Ok((inp, value))
}

fn parse_line(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        separated_pair(
            separated_pair(
                double, 
                multispace1, 
                double
            ),
            multispace1, 
            char('l')
        ), |value| ContentToken::Line(value.0))(start_inp)?;

    Ok((inp, value))
}

fn parse_move(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        separated_pair(
            separated_pair(
                double, 
                multispace1, 
                double
            ),
            multispace1, 
            char('m')
        ), |value| ContentToken::Move(value.0))(start_inp)?;

    Ok((inp, value))
}

fn parse_line_width(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(separated_pair(double, multispace1, char('w')), |value| ContentToken::LineWidth(value.0))(start_inp)?;

    Ok((inp, value))
}

fn parse_color_space_grey(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(separated_pair(double, multispace1, char('g')), |value| ContentToken::ColorSpaceGrey(value.0))(start_inp)?;

    Ok((inp, value))
}

fn parse_g(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(separated_pair(double, multispace1, char('G')), |value| ContentToken::StrokingColorSpaceGrey(value.0))(start_inp)?;

    Ok((inp, value))
}

fn parse_flatness_tolerance(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(separated_pair(double, multispace1, char('i')), |value| ContentToken::SetFlatnessTolerance(value.0))(start_inp)?;

    Ok((inp, value))
}

fn parse_bmc(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        separated_pair(
            parse_tag,
            multispace0,
            tag("BMC")
        ),
        |value| ContentToken::BeginMarkedContent(String::from_utf8_lossy(value.0).to_string()))(start_inp)?;

    Ok((inp, value))
}

fn parse_begin_text_object(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        tag("BT"),
        |value| ContentToken::BeginTextObject)(start_inp)?;

    Ok((inp, value))
}

fn parse_end_text_object(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        tag("ET"),
        |value| ContentToken::EndTextObject)(start_inp)?;

    Ok((inp, value))
}

fn parse_end_marked_content(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(delimited(multispace0, tag("EMC"), multispace1), |value| ContentToken::EndMarkedContent)(start_inp)?;

    Ok((inp, value))
}

fn parse_end_path(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(delimited(multispace0, char('n'), multispace1), |value| ContentToken::EndPath)(start_inp)?;

    Ok((inp, value))
}

fn parse_cm(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        pair(
            count(
                delimited(
                    multispace0,
                    double,
                    multispace0
                ),
                6
            ),
            tag("cm")),
             |value| ContentToken::Cm(value.0)
        )(start_inp)?;

    // dbg!(&value);

    Ok((inp, value))
}

fn parse_set_text_matrix(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        pair(
            count(
                delimited(
                    multispace0,
                    double,
                    multispace0
                ),
                6
            ),
            tag("Tm")),
             |value| ContentToken::SetTextMatrix(value.0)
        )(start_inp)?;

    Ok((inp, value))
}

fn parse_set_text_font(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        tuple((
            parse_tag,
            delimited(
                multispace0,
                double,
                multispace1
            ),
            tag("Tf"))),
             |value| value
        )(start_inp)?;
    

    // ContentToken::TextFont
    let font = String::from_utf8(value.0.to_vec()).unwrap();
    let font_size = value.1;
    let value = ContentToken::TextFont((font, font_size));

    Ok((inp, value))
}

fn parse_show_text_string(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        separated_pair(
            parse_string,
            multispace0,
            tag("Tj"),
        ),
             |value| ContentToken::ShowTextString(String::from_utf8_lossy(value.0).to_string())
        )(start_inp)?;
    
    // dbg!(&value);

    Ok((inp, value))
}

fn parse_fill_path_even_odd(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(delimited(multispace0, tag("f*"), multispace1), |value| ContentToken::FillPathEvenOdd)(start_inp)?;

    Ok((inp, value))
}

fn parse_save_graphics_state(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(delimited(multispace0, char('q'), multispace1), |value| ContentToken::SaveGraphicsState)(start_inp)?;

    Ok((inp, value))
}

fn parse_restore_graphics_state(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(delimited(multispace0, char('Q'), multispace1), |value| ContentToken::RestoreGraphicsState)(start_inp)?;

    Ok((inp, value))
}

fn parse_paint_x_object(start_inp: &[u8]) -> IResult<&[u8], ContentToken> {
    let (inp, value) = map(
        separated_pair(
            parse_tag, 
            multispace0, 
            tag("Do")
        ), |value| ContentToken::PaintXObject(String::from_utf8(value.0.to_vec()).unwrap()))(start_inp)?;

    Ok((inp, value))
}

pub fn parse(source: &[u8]) -> Vec<ContentToken> {
    // let result = many0(
    //     alt((
    //         parse_cm,
    //         parse_bmc
    //     )))(source);

    // let result: IResult<&[u8], Vec<ContentToken>> = many0(
    //     delimited(alt((multispace0, newline)), alt((
    //         parse_cm,
    //         parse_bmc
    //     )), alt((multispace0, newline)))
    // )(source);

    // let a= parse_string("(Account)Tj".as_bytes());
    // dbg!(a);
    // panic!();

    // let result: IResult<&[u8], Vec<ContentToken>> = many0(
    //     delimited(
    //         multispace0,
    //         alt((
    //             parse_cm,
    //             parse_bmc,
    //             parse_end_marked_content,
    //             parse_g,
    //             parse_line_width,
    //             parse_move,
    //             parse_line,
    //             parse_stroke_path,
    //             parse_bdc,
    //             parse_color_space_grey,
    //             parse_begin_text_object,
    //             parse_end_text_object,
    //             parse_set_text_matrix,
    //             parse_set_text_font,
    //             parse_show_text_string,
    //             parse_flatness_tolerance,
    //             parse_end_path,
    //             parse_fill_path_even_odd,
    //             parse_save_graphics_state,
    //             parse_restore_graphics_state,
    //             parse_paint_x_object
    //         )), 
    //         multispace0)
    // )(source);

    // dbg!(String::from_utf8_lossy(source));
    // panic!();

    let result: IResult<&[u8], Vec<ContentToken>> = many0(
        delimited(
            multispace0,
            alt((
                parse_cm,
                parse_bmc,
                parse_end_marked_content,
                parse_g,
                parse_line_width,
                parse_move,
                parse_line,
                parse_stroke_path,
                parse_bdc,
                parse_color_space_grey,
                parse_begin_text_object,
                parse_end_text_object,
                parse_set_text_matrix,
                parse_set_text_font,
                parse_show_text_string,
                parse_flatness_tolerance,
                parse_end_path,
                parse_fill_path_even_odd,
                parse_save_graphics_state,
                parse_restore_graphics_state,
                parse_paint_x_object
            )), 
            multispace0)
    )(source);
    
    let result = result.unwrap();
    // dbg!(result.unwrap().1);

    result.1

    // let result = many0(alt(
    //     parse_cm
    // ))(source)?;
    // let (source2, items) = many0(alt((
    //     Value::parse_bytes,
    //     Value::parse_integer,
    //     Value::parse_list,
    //     Value::parse_dict,
    // )))(source)?;
    // dbg!(result);

    // let _ = eof(source2)?;

    // Ok(items)
}
