use crate::cube::*;

use nom::types::CompleteStr as Input;
use nom::*;

named!(layer<Input, Layer>,
    do_parse!(
        base: one_of!("UFRLBDMESufrlbdxyz") >>
        wide: opt!(char!('w')) >>
        (match (base, wide.is_some()) {
            // SiGN notation (lowercase already means wide) and Uw/Fw/... suffix notation
            ('U', false) => Layer::U,
            ('U', true) | ('u', _) => Layer::Uw,
            ('F', false) => Layer::F,
            ('F', true) | ('f', _) => Layer::Fw,
            ('R', false) => Layer::R,
            ('R', true) | ('r', _) => Layer::Rw,
            ('L', false) => Layer::L,
            ('L', true) | ('l', _) => Layer::Lw,
            ('B', false) => Layer::B,
            ('B', true) | ('b', _) => Layer::Bw,
            ('D', false) => Layer::D,
            ('D', true) | ('d', _) => Layer::Dw,
            ('M', _) => Layer::M,
            ('E', _) => Layer::E,
            ('S', _) => Layer::S,
            ('x', _) => Layer::X,
            ('y', _) => Layer::Y,
            ('z', _) => Layer::Z,
            _ => unreachable!()
        })
    )
);

named!(order<Input, Order>,
    map!(
        opt!(one_of!("'2")),
        |or| match or {
            Some('\'') => Order::Prime,
            Some('2') => Order::Double,
            None => Order::Normal,
            _ => unreachable!()
        }
    )
);

named!(move_<Input, Move>,
    do_parse!(
        multispace0 >> layer: layer >>
        multispace0 >> order: order >>
        (Move { order, layer })
    )
);

named!(moves<Input, Vec<Move>>,
    many0!( move_ )
);

pub fn parse_moves(data: &str) -> Result<Vec<Move>, String> {
    let (etc, moves) = moves(Input(data)).expect("unknown parser error");
    let etc = etc.to_string();
    if !etc.is_empty() {
        let pos = data.len() - etc.len();
        Err(format!(
            "{}\n{}^ parse error at position {}",
            data,
            String::from_utf8(vec![b' '; pos]).unwrap(),
            pos + 1,
        ))
    } else {
        Ok(moves)
    }
}
