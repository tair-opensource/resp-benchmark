use nom::{
    IResult,
    sequence::delimited,
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::multispace0,
    multi::many0,
    combinator::{map, all_consuming},
};
use crate::command::placeholder::PlaceholderEnum;

fn parse_string(input: &str) -> IResult<&str, PlaceholderEnum> {
    let s = alt((
        delimited(tag("\""), is_not("\""), tag("\"")),
        delimited(tag("\'"), is_not("\'"), tag("\'")),
        delimited(multispace0, is_not("{ "), multispace0)
        ));
    map(s, PlaceholderEnum::new_string)(input)
}


fn parse_placeholder(input: &str) -> IResult<&str, PlaceholderEnum> {
    let inner = delimited(tag("{"), is_not("}"), tag("}"));
    let eat_whitespace = delimited(multispace0, inner, multispace0);
    map(eat_whitespace, PlaceholderEnum::new)(input)
}


pub(crate) fn parse_all(input: &str) -> IResult<&str, Vec<PlaceholderEnum>> {
    all_consuming(many0(alt((parse_string, parse_placeholder))))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root() {
        let (nm, args) = match parse_all("aa {key sequence 100} bbb") {
            Ok((nm, args)) => (nm, args),
            Err(e) => {
                println!("Error: {:?}", e);
                return;
            }
        };
        println!("nm: {:?}, args: {:?}", nm, args);
    }
}