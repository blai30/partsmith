//! The check of a STEP file's bytes before OCCT reads them (kernel spec section 3.8.1 read items 6
//! and 7, ADR 0073).
//!
//! OCCT 7.9.3 follows instance references by recursion. A reference cycle or a long chain of
//! references overflows its stack. Its parser recovers from a broken parameter list by leaving
//! records that later read as null entities. A number defined twice sends its numbering of the
//! records into a second pass that reads memory it never set. This check reads the bytes with the
//! rules of OCCT's lexer (`src/StepFile/step.lex` of V7_9_3) and refuses such a file before OCCT
//! sees it. It never recurses, and its memory grows with the bytes alone.

use std::collections::HashMap;

/// The largest instance number OCCT 7.9.3 reads: it parses the digits after `#` with `atoi` into an
/// `int`.
const LARGEST_INSTANCE_NUMBER: u32 = 2_147_483_647;

/// The most references a chain of instance references may hold (ADR 0073 choice 2).
const LONGEST_CHAIN: usize = 500;

/// Returns the raw message of the first rule of read items 6 and 7 that the bytes break: in the
/// order of the text, the first parameter list that breaks STEP's syntax, instance number larger
/// than OCCT reads, number defined a second time or scope; then the first reference cycle the walk
/// closes; then the first chain longer than [`LONGEST_CHAIN`] that the walk finishes.
pub(crate) fn check_step_bytes(bytes: &[u8]) -> Result<(), String> {
    read_instances(bytes)?.walk()
}

/// What a token of OCCT's lexer is, for the check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// `(`.
    Open,
    /// `)`.
    Close,
    /// `,`.
    Comma,
    /// `#<digits>` followed by blanks and `=`: the definition of an instance.
    Definition,
    /// Any other `#<digits>`: a reference.
    Reference,
    /// A word, such as an entity type: OCCT's `TYPE` token.
    Word,
    /// A parameter of one token: a number, a string, an enumeration, a binary, `$` and every
    /// character no other rule reads, `*` among them.
    Value,
    /// `&SCOPE`.
    Scope,
    /// `ENDSCOPE`.
    EndScope,
    /// `;`, `=`, `/` and the keywords of the sections.
    Other,
    /// `END-ISO...;`, after which the lexer reads nothing.
    End,
}

/// One token: its kind, its bytes and the line it starts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
    line: usize,
}

/// Reads tokens as OCCT's lexer does. Comments, blanks and line ends are skipped.
struct Lexer<'a> {
    bytes: &'a [u8],
    position: usize,
    line: usize,
}

/// Whether `byte` continues a word (`[a-zA-Z0-9_]`).
fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whether `byte` may end a string's apostrophe before `)` or `,` (`[" "\n\r]` of step.lex).
fn is_string_end_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'"' | b'\n' | b'\r')
}

impl<'a> Lexer<'a> {
    fn new(bytes: &'a [u8]) -> Lexer<'a> {
        Lexer {
            bytes,
            position: 0,
            line: 1,
        }
    }

    fn at(&self, index: usize) -> Option<u8> {
        self.bytes.get(index).copied()
    }

    /// The number of bytes from `start` on that `accept` takes.
    fn run(&self, start: usize, accept: impl Fn(u8) -> bool) -> usize {
        self.bytes
            .get(start..)
            .unwrap_or_default()
            .iter()
            .take_while(|&&byte| accept(byte))
            .count()
    }

    /// Whether the bytes at `start` are `text`, in any case.
    fn starts_with(&self, start: usize, text: &[u8]) -> bool {
        self.bytes
            .get(start..start.saturating_add(text.len()))
            .is_some_and(|bytes| bytes.eq_ignore_ascii_case(text))
    }

    /// Counts the line ends of `bytes[start..end]`.
    fn count_lines(&mut self, start: usize, end: usize) {
        let bytes = self.bytes.get(start..end).unwrap_or_default();
        self.line += bytes.iter().filter(|&&byte| byte == b'\n').count();
    }

    /// Skips a comment from its `/*` to the first `*/`, or to the end of the bytes.
    fn skip_comment(&mut self) {
        let start = self.position + 2;
        let rest = self.bytes.get(start..).unwrap_or_default();
        let end = rest
            .windows(2)
            .position(|pair| pair == b"*/")
            .map_or(self.bytes.len(), |offset| start + offset + 2);
        self.count_lines(start, end);
        self.position = end;
    }

    /// Skips a string from its opening apostrophe. It ends at an apostrophe that blanks of
    /// [`is_string_end_blank`] and then `)` or `,` follow, or at the end of the bytes. Any other
    /// apostrophe, a doubled one too, stays in the string.
    fn skip_string(&mut self) {
        let mut index = self.position + 1;
        while let Some(offset) = self
            .bytes
            .get(index..)
            .and_then(|rest| rest.iter().position(|&byte| byte == b'\''))
        {
            let apostrophe = index + offset;
            self.count_lines(index, apostrophe);
            let after = apostrophe + 1;
            let blanks = self.run(after, is_string_end_blank);
            if matches!(self.at(after + blanks), Some(b')' | b',')) {
                self.position = after;
                return;
            }
            index = after;
        }
        self.count_lines(index, self.bytes.len());
        self.position = self.bytes.len();
    }

    /// The kind and length of the longest token of step.lex's other rules at `start`. A tie goes
    /// to the rule step.lex lists first, as in flex.
    fn longest_match(&self, start: usize) -> (Kind, usize) {
        let first = self.at(start).unwrap_or_default();
        let digits = |from: usize| self.run(from, |byte| byte.is_ascii_digit());
        let mut best = (Kind::Value, 0);
        let mut offer = |kind: Kind, length: usize| {
            if length > best.1 {
                best = (kind, length);
            }
        };
        // `[-+0-9][0-9]*`, `[-+\.0-9][\.0-9]+` and `[-+\.0-9][\.0-9]+E[-+0-9][0-9]*`.
        if matches!(first, b'-' | b'+') || first.is_ascii_digit() {
            offer(Kind::Value, 1 + digits(start + 1));
        }
        if matches!(first, b'-' | b'+' | b'.') || first.is_ascii_digit() {
            let mantissa = self.run(start + 1, |byte| byte == b'.' || byte.is_ascii_digit());
            if mantissa > 0 {
                let real = 1 + mantissa;
                offer(Kind::Value, real);
                let exponent = start + real;
                let signed = self
                    .at(exponent + 1)
                    .is_some_and(|byte| matches!(byte, b'-' | b'+') || byte.is_ascii_digit());
                if matches!(self.at(exponent), Some(b'E' | b'e')) && signed {
                    offer(Kind::Value, real + 2 + digits(exponent + 2));
                }
            }
        }
        // `["][0-9A-F]+["]`, a binary.
        if first == b'"' {
            let hexadecimal = self.run(start + 1, |byte| byte.is_ascii_hexdigit());
            if hexadecimal > 0 && self.at(start + 1 + hexadecimal) == Some(b'"') {
                offer(Kind::Value, hexadecimal + 2);
            }
        }
        // `[.]*[A-Z0-9_]+[.]`, an enumeration.
        let dots = self.run(start, |byte| byte == b'.');
        let word = self.run(start + dots, is_word);
        if word > 0 && self.at(start + dots + word) == Some(b'.') {
            offer(Kind::Value, dots + word + 1);
        }
        match first {
            b'(' => offer(Kind::Open, 1),
            b')' => offer(Kind::Close, 1),
            b',' => offer(Kind::Comma, 1),
            b'$' => offer(Kind::Value, 1),
            b'=' | b';' => offer(Kind::Other, 1),
            _ => {}
        }
        // Every keyword starts with a letter.
        if first.is_ascii_alphabetic() {
            for keyword in [&b"STEP;"[..], b"HEADER;", b"ENDSEC;", b"DATA;"] {
                if self.starts_with(start, keyword) {
                    offer(Kind::Other, keyword.len());
                }
            }
            // `ENDSTEP;.*` takes the rest of the line.
            if self.starts_with(start, b"ENDSTEP;") {
                offer(Kind::Other, 8 + self.run(start + 8, |byte| byte != b'\n'));
            }
            // `END-ISO[0-9\-]*;` and `ISO[0-9\-]*;`.
            for (keyword, kind) in [(&b"END-ISO"[..], Kind::End), (b"ISO", Kind::Other)] {
                if self.starts_with(start, keyword) {
                    let after = start + keyword.len();
                    let numbers = self.run(after, |byte| byte == b'-' || byte.is_ascii_digit());
                    if self.at(after + numbers) == Some(b';') {
                        offer(kind, keyword.len() + numbers + 1);
                    }
                }
            }
        }
        if first == b'/' {
            offer(Kind::Other, 1);
        }
        if first == b'&' && self.starts_with(start, b"&SCOPE") {
            offer(Kind::Scope, 6);
        }
        if self.starts_with(start, b"ENDSCOPE") {
            offer(Kind::EndScope, 8);
        }
        // `[a-zA-Z0-9_]+` and `![a-zA-Z0-9_]+`.
        offer(Kind::Word, self.run(start, is_word));
        if first == b'!' {
            let bang = self.run(start + 1, is_word);
            if bang > 0 {
                offer(Kind::Word, bang + 1);
            }
        }
        // `[^)]`: one character of any other kind.
        offer(Kind::Value, 1);
        best
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        loop {
            let start = self.position;
            let line = self.line;
            let token = |kind, end| Token {
                kind,
                start,
                end,
                line,
            };
            match self.at(start)? {
                b'\n' => {
                    self.line += 1;
                    self.position += 1;
                }
                b' ' | b'\t' | b'\r' | 0 => self.position += 1,
                b'/' if self.at(start + 1) == Some(b'*') => self.skip_comment(),
                b'\'' => {
                    self.skip_string();
                    return Some(token(Kind::Value, self.position));
                }
                b'#' if self.at(start + 1).is_some_and(|byte| byte.is_ascii_digit()) => {
                    let end = start + 1 + self.run(start + 1, |byte| byte.is_ascii_digit());
                    let blanks = self.run(end, |byte| matches!(byte, b' ' | b'\t'));
                    let kind = if self.at(end + blanks) == Some(b'=') {
                        Kind::Definition
                    } else {
                        Kind::Reference
                    };
                    self.position = end;
                    return Some(token(kind, end));
                }
                _ => {
                    let (kind, length) = self.longest_match(start);
                    if kind == Kind::End {
                        self.position = self.bytes.len();
                        return None;
                    }
                    self.position = start + length;
                    return Some(token(kind, self.position));
                }
            }
        }
    }
}

/// Where a parameter list is in its grammar, `(` [parameter {`,` parameter}] `)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListPlace {
    /// Right after `(`: a parameter or `)` may follow.
    Opened,
    /// Right after `,`: a parameter must follow.
    AfterComma,
    /// Right after a parameter: `,` or `)` must follow.
    AfterParameter,
}

/// The check of read item 7: the parameter lists open at a token, innermost last.
///
/// A parameter list starts at a `(` right after a word, as an entity's own list and a typed
/// parameter do. Its parameters are references, one-token values, lists and typed parameters,
/// one between each two commas.
#[derive(Debug, Default)]
struct Lists {
    open: Vec<ListPlace>,
    after_word: bool,
}

impl Lists {
    /// Reads the next token, or returns the raw message of the list it breaks.
    fn read(&mut self, token: Token) -> Result<(), String> {
        let after_word = std::mem::replace(&mut self.after_word, false);
        if after_word && token.kind == Kind::Open {
            self.open.push(ListPlace::Opened);
            return Ok(());
        }
        let Some(place) = self.open.last_mut() else {
            self.after_word = token.kind == Kind::Word;
            return Ok(());
        };
        if after_word {
            return Err(syntax_message(token.line));
        }
        match (token.kind, *place) {
            (Kind::Close, ListPlace::Opened | ListPlace::AfterParameter) => {
                self.open.pop();
            }
            (Kind::Comma, ListPlace::AfterParameter) => *place = ListPlace::AfterComma,
            (Kind::Open, ListPlace::Opened | ListPlace::AfterComma) => {
                *place = ListPlace::AfterParameter;
                self.open.push(ListPlace::Opened);
            }
            (
                Kind::Reference | Kind::Value | Kind::Word,
                ListPlace::Opened | ListPlace::AfterComma,
            ) => {
                *place = ListPlace::AfterParameter;
                self.after_word = token.kind == Kind::Word;
            }
            _ => return Err(syntax_message(token.line)),
        }
        Ok(())
    }
}

/// The raw message of a parameter list that breaks STEP's syntax (read item 7).
fn syntax_message(line: usize) -> String {
    format!("the STEP file breaks STEP's syntax inside a parameter list on line {line}")
}

/// The value of the digits of an instance number, or the raw message of read item 6 when it is
/// larger than [`LARGEST_INSTANCE_NUMBER`].
fn instance_number(digits: &[u8]) -> Result<u32, String> {
    digits
        .iter()
        .try_fold(0u32, |value, &digit| {
            value
                .checked_mul(10)?
                .checked_add(u32::from(digit - b'0'))
                .filter(|&value| value <= LARGEST_INSTANCE_NUMBER)
        })
        .ok_or_else(|| {
            format!(
                "the STEP file's instance number #{} is larger than {LARGEST_INSTANCE_NUMBER}",
                String::from_utf8_lossy(digits)
            )
        })
}

/// The raw message of an instance number that the file defines a second time (read item 6).
fn repeated_message(number: u32, line: usize) -> String {
    format!(
        "the STEP file defines the instance number #{number} more than once, again on line {line}"
    )
}

/// The raw message of a scope (read item 6).
fn scope_message(line: usize) -> String {
    format!("the STEP file opens or closes a scope on line {line}")
}

/// The instances of a file in the order it defines them, with the references of each.
#[derive(Debug, Default)]
struct Instances {
    /// The number of each instance.
    numbers: Vec<u32>,
    /// The instance of each number.
    by_number: HashMap<u32, usize>,
    /// Each reference, in the order of the text: the instance whose text holds it and the number
    /// it names.
    references: Vec<(usize, u32)>,
}

/// Reads the instances and their references. On the way it checks the syntax of every parameter
/// list ([`Lists`]), the size of every instance number, that no number is defined twice and that
/// no scope opens or closes (read items 6 and 7). A reference belongs to the instance whose
/// definition comes last before it.
fn read_instances(bytes: &[u8]) -> Result<Instances, String> {
    let mut instances = Instances::default();
    let mut current: Option<usize> = None;
    let mut lists = Lists::default();
    for token in Lexer::new(bytes) {
        lists.read(token)?;
        let digits = bytes.get(token.start + 1..token.end).unwrap_or_default();
        match token.kind {
            Kind::Definition => {
                let number = instance_number(digits)?;
                let instance = instances.numbers.len();
                if instances.by_number.insert(number, instance).is_some() {
                    return Err(repeated_message(number, token.line));
                }
                instances.numbers.push(number);
                current = Some(instance);
            }
            Kind::Reference => {
                let number = instance_number(digits)?;
                if let Some(instance) = current {
                    instances.references.push((instance, number));
                }
            }
            Kind::Scope | Kind::EndScope => return Err(scope_message(token.line)),
            _ => {}
        }
    }
    Ok(instances)
}

/// The raw message of a cycle (read item 6): `head` is the number of its instance the walk
/// reached first, and `references` how many references it runs through.
fn cycle_message(head: u32, references: usize) -> String {
    let noun = if references == 1 {
        "reference"
    } else {
        "references"
    };
    format!(
        "the STEP file's instance references form a cycle: #{head} reaches itself through {references} {noun}"
    )
}

/// The raw message of a chain longer than [`LONGEST_CHAIN`] (read item 6), from the instance
/// numbered `start`.
fn chain_message(start: u32) -> String {
    format!(
        "the STEP file's instance references form a chain of more than {LONGEST_CHAIN} references from #{start}"
    )
}

/// Where the walk is with an instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visit {
    /// The walk has not reached it.
    Unreached,
    /// It is on the walk's stack.
    OnStack,
    /// The walk has followed all its references.
    Finished,
}

/// One instance on the walk's stack, with the next of its targets to follow and the longest
/// chain found from it so far.
struct Frame {
    instance: usize,
    next: usize,
    chain: usize,
}

impl Instances {
    /// The graph of the references, in compressed rows: the instances `0..n` in file order, and
    /// for each the instances its references name, in the order of its text. A reference to a
    /// number the file does not define leads nowhere.
    ///
    /// Returns the start of each instance's targets in the second list, with one more entry at
    /// the end.
    fn graph(&self) -> (Vec<usize>, Vec<usize>) {
        let resolved: Vec<(usize, usize)> = self
            .references
            .iter()
            .filter_map(|&(instance, number)| Some((instance, *self.by_number.get(&number)?)))
            .collect();
        let mut starts = vec![0; self.numbers.len() + 1];
        for &(instance, _) in &resolved {
            starts[instance + 1] += 1;
        }
        for instance in 1..starts.len() {
            starts[instance] += starts[instance - 1];
        }
        let mut targets = vec![0; resolved.len()];
        let mut filled = starts.clone();
        for (instance, target) in resolved {
            targets[filled[instance]] = target;
            filled[instance] += 1;
        }
        (starts, targets)
    }

    /// Walks the graph depth first with an explicit stack: from each instance in file order that
    /// it has not reached yet, following each instance's targets in order. Refuses the first
    /// cycle it closes, then the first instance whose longest chain holds more than
    /// [`LONGEST_CHAIN`] references, in the order the walk finishes them.
    fn walk(&self) -> Result<(), String> {
        let count = self.numbers.len();
        let (starts, targets) = self.graph();
        let mut visits = vec![Visit::Unreached; count];
        let mut position = vec![0; count];
        let mut chain = vec![0; count];
        let mut stack: Vec<Frame> = Vec::new();
        for root in 0..count {
            if visits[root] != Visit::Unreached {
                continue;
            }
            visits[root] = Visit::OnStack;
            stack.push(Frame {
                instance: root,
                next: starts[root],
                chain: 0,
            });
            while let Some(top) = stack.last_mut() {
                if top.next < starts[top.instance + 1] {
                    let target = targets[top.next];
                    top.next += 1;
                    match visits[target] {
                        Visit::Unreached => {
                            visits[target] = Visit::OnStack;
                            position[target] = stack.len();
                            stack.push(Frame {
                                instance: target,
                                next: starts[target],
                                chain: 0,
                            });
                        }
                        Visit::OnStack => {
                            let head = stack[position[target]].instance;
                            let references = stack.len() - position[target];
                            return Err(cycle_message(self.numbers[head], references));
                        }
                        Visit::Finished => top.chain = top.chain.max(chain[target] + 1),
                    }
                    continue;
                }
                let Some(done) = stack.pop() else { break };
                visits[done.instance] = Visit::Finished;
                chain[done.instance] = done.chain;
                if done.chain > LONGEST_CHAIN {
                    return Err(chain_message(self.numbers[done.instance]));
                }
                if let Some(parent) = stack.last_mut() {
                    parent.chain = parent.chain.max(done.chain + 1);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kinds of the tokens of `text`.
    fn kinds(text: &str) -> Vec<Kind> {
        Lexer::new(text.as_bytes())
            .map(|token| token.kind)
            .collect()
    }

    /// A file of `lines` in the form of kernel spec test 173's input.
    fn step(lines: &[&str]) -> String {
        format!(
            "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
            lines.join("\n")
        )
    }

    /// The result of the check of `lines` in a file.
    fn checked(lines: &[&str]) -> Result<(), String> {
        check_step_bytes(step(lines).as_bytes())
    }

    /// The oriented edge `#number` whose edge element is `#element`.
    fn oriented_edge(number: usize, element: usize) -> String {
        format!("#{number}=ORIENTED_EDGE('',*,*,#{element},.F.);")
    }

    #[test]
    fn a_comment_is_skipped() {
        assert_eq!(kinds("/* #1 'x */ #2"), [Kind::Reference]);
    }

    #[test]
    fn a_comment_without_its_end_runs_to_the_end_of_the_bytes() {
        assert_eq!(kinds("#1 /* #2"), [Kind::Reference]);
    }

    #[test]
    fn a_string_is_one_value() {
        assert_eq!(
            kinds("('#1',#2)"),
            [
                Kind::Open,
                Kind::Value,
                Kind::Comma,
                Kind::Reference,
                Kind::Close
            ]
        );
    }

    #[test]
    fn a_doubled_apostrophe_stays_in_its_string() {
        assert_eq!(
            kinds("('it''s #1')"),
            [Kind::Open, Kind::Value, Kind::Close]
        );
    }

    #[test]
    fn an_apostrophe_before_a_letter_stays_in_its_string() {
        assert_eq!(kinds("('it's #1')"), [Kind::Open, Kind::Value, Kind::Close]);
    }

    #[test]
    fn a_string_ends_before_blanks_and_line_ends_then_a_parenthesis() {
        assert_eq!(
            kinds("('x' \r\n) #1"),
            [Kind::Open, Kind::Value, Kind::Close, Kind::Reference]
        );
    }

    #[test]
    fn a_tab_before_the_comma_keeps_the_string_open() {
        assert_eq!(kinds("('x'\t, #1"), [Kind::Open, Kind::Value]);
    }

    #[test]
    fn a_comment_opener_inside_a_string_opens_no_comment() {
        assert_eq!(kinds("('/*',#1)").len(), 5);
    }

    #[test]
    fn blanks_and_tabs_before_the_equals_sign_define_an_instance() {
        assert_eq!(kinds("#1 \t =X()")[0], Kind::Definition);
    }

    #[test]
    fn a_line_end_before_the_equals_sign_makes_a_reference() {
        assert_eq!(kinds("#1\n=X()")[0], Kind::Reference);
    }

    #[test]
    fn nothing_after_the_end_line_is_read() {
        assert_eq!(kinds("#1 END-ISO-10303-21; #2"), [Kind::Reference]);
    }

    #[test]
    fn the_end_line_is_read_in_any_case() {
        assert!(kinds("end-iso-10303-21; #2").is_empty());
    }

    #[test]
    fn the_end_keyword_inside_a_word_is_a_word() {
        assert_eq!(kinds("XEND-ISO; #2")[0], Kind::Word);
        assert_eq!(*kinds("XEND-ISO; #2").last().unwrap(), Kind::Reference);
    }

    #[test]
    fn the_old_end_keyword_takes_the_rest_of_its_line() {
        assert_eq!(kinds("ENDSTEP; #1\n#2"), [Kind::Other, Kind::Reference]);
    }

    #[test]
    fn numbers_reals_and_enumerations_are_values() {
        let text = "(1,-2.5,1.E-07,.T.,\"0F\",$,*)";
        assert!(
            kinds(text)
                .iter()
                .all(|kind| !matches!(kind, Kind::Word | Kind::Other)),
            "{:?}",
            kinds(text)
        );
    }

    #[test]
    fn a_number_followed_by_letters_is_a_word() {
        assert_eq!(kinds("12AB"), [Kind::Word]);
    }

    #[test]
    fn lines_are_counted_inside_strings_and_comments() {
        let tokens: Vec<Token> = Lexer::new(b"('a\nb')\n/*\n*/ #1").collect();

        assert_eq!(tokens.last().map(|token| token.line), Some(4));
    }

    #[test]
    fn a_valid_file_passes() {
        let lines = [
            "#1=EDGE_LOOP('',(#2));",
            "#2=ORIENTED_EDGE('',*,*,#3,.F.);",
            "#3=EDGE_CURVE('',#4,#4,#5,.T.);",
            "#4=VERTEX_POINT('',#6);",
            "#5=LINE('',#6,#7);",
            "#6=CARTESIAN_POINT('',(0.,0.,0.));",
            "#7=VECTOR('',#8,1.);",
            "#8=DIRECTION('',(0.,0.,1.));",
        ];

        assert_eq!(checked(&lines), Ok(()));
    }

    #[test]
    fn a_typed_parameter_and_an_empty_list_pass() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL(LENGTH_MEASURE(1.),());"]),
            Ok(())
        );
    }

    #[test]
    fn a_complex_instance_passes() {
        assert_eq!(
            checked(&[
                "#1=(PARTSMITH_FIRST(#2) PARTSMITH_SECOND(1.));",
                "#2=VERTEX('');"
            ]),
            Ok(())
        );
    }

    #[test]
    fn an_empty_parameter_is_refused_with_its_line() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL('x',(1,,2));"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn a_comma_before_the_closing_parenthesis_is_refused() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL(1,);"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn a_comma_right_after_the_opening_parenthesis_is_refused() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL(,1);"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn a_word_without_its_list_inside_a_list_is_refused() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL(O(1),(FOO,(1)));"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn two_parameters_without_a_comma_are_refused() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL(1 2);"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn a_definition_inside_a_list_is_refused() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL((#2=VERTEX('')));"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn a_semicolon_inside_a_list_is_refused() {
        assert_eq!(checked(&["#1=PARTSMITH_LABEL((1;"]), Err(syntax_message(5)));
    }

    #[test]
    fn a_list_in_the_header_is_checked() {
        let text =
            "ISO-10303-21;\nHEADER;\nFILE_NAME((,));\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";

        assert_eq!(check_step_bytes(text.as_bytes()), Err(syntax_message(3)));
    }

    #[test]
    fn parentheses_outside_a_list_are_not_checked() {
        // The broken line of delivery.md section 3.8 finding 1 is left to OCCT's own check list.
        assert_eq!(
            checked(&["#('2D SPACE',''", "  ) );", "#1=VERTEX('');"]),
            Ok(())
        );
    }

    #[test]
    fn a_self_reference_is_a_cycle_of_one_reference() {
        assert_eq!(checked(&[&oriented_edge(2, 2)]), Err(cycle_message(2, 1)));
    }

    #[test]
    fn a_cycle_names_the_instance_the_walk_reached_first() {
        let lines = [
            oriented_edge(5, 6),
            oriented_edge(6, 7),
            oriented_edge(7, 6),
        ];

        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(checked(&lines), Err(cycle_message(6, 2)));
    }

    #[test]
    fn the_walk_starts_from_instances_in_file_order() {
        let lines = [
            oriented_edge(9, 8),
            oriented_edge(8, 9),
            oriented_edge(1, 1),
        ];

        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(checked(&lines), Err(cycle_message(9, 2)));
    }

    #[test]
    fn references_are_followed_in_the_order_of_the_text() {
        let lines = [
            "#1=EDGE_LOOP('',(#3,#2));".to_owned(),
            oriented_edge(2, 2),
            oriented_edge(3, 3),
        ];

        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(checked(&lines), Err(cycle_message(3, 1)));
    }

    #[test]
    fn a_reference_to_a_missing_instance_leads_nowhere() {
        assert_eq!(checked(&[&oriented_edge(2, 3)]), Ok(()));
    }

    #[test]
    fn references_before_the_first_definition_belong_to_no_instance() {
        let text = "ISO-10303-21;\nHEADER;\nPARTSMITH_LABEL(#1);\nENDSEC;\nDATA;\n#1=PARTSMITH_LABEL(1.);\nENDSEC;\nEND-ISO-10303-21;\n";

        assert_eq!(check_step_bytes(text.as_bytes()), Ok(()));
    }

    #[test]
    fn a_number_defined_twice_is_refused_at_its_second_definition() {
        assert_eq!(
            checked(&["#2=VERTEX('');", "#3=VERTEX('');", "#2=VERTEX('');"]),
            Err(repeated_message(2, 7))
        );
    }

    #[test]
    fn leading_zeros_define_the_same_number() {
        assert_eq!(
            checked(&["#2=VERTEX('');", "#002=VERTEX('');"]),
            Err(repeated_message(2, 6))
        );
    }

    #[test]
    fn a_repeated_number_is_refused_before_a_cycle_later_in_the_text() {
        assert_eq!(
            checked(&["#3=VERTEX('');", "#3=VERTEX('');", &oriented_edge(2, 2)]),
            Err(repeated_message(3, 6))
        );
    }

    #[test]
    fn a_scope_is_refused_with_its_line() {
        assert_eq!(
            checked(&[
                "#1=&SCOPE",
                "#2=VERTEX('');",
                "ENDSCOPE PARTSMITH_LABEL(1.);"
            ]),
            Err(scope_message(5))
        );
    }

    #[test]
    fn an_end_of_scope_alone_is_refused() {
        assert_eq!(
            checked(&["#1=VERTEX('');", "ENDSCOPE / #1 /"]),
            Err(scope_message(6))
        );
    }

    #[test]
    fn a_scope_keyword_inside_a_list_breaks_its_syntax() {
        assert_eq!(
            checked(&["#1=PARTSMITH_LABEL((&SCOPE));"]),
            Err(syntax_message(5))
        );
    }

    #[test]
    fn leading_zeros_name_the_same_instance() {
        assert_eq!(
            checked(&["#2=ORIENTED_EDGE('',*,*,#0002,.F.);"]),
            Err(cycle_message(2, 1))
        );
    }

    #[test]
    fn the_largest_instance_number_is_read() {
        assert_eq!(instance_number(b"2147483647"), Ok(2_147_483_647));
    }

    #[test]
    fn a_larger_instance_number_is_refused_with_its_digits() {
        assert_eq!(
            instance_number(b"2147483648"),
            Err("the STEP file's instance number #2147483648 is larger than 2147483647".to_owned())
        );
    }

    #[test]
    fn an_instance_number_longer_than_64_bits_is_refused() {
        let digits = "9".repeat(40);

        let message = instance_number(digits.as_bytes()).expect_err("too large");

        assert!(message.contains(&digits), "{message}");
    }

    /// A chain of `references` references: oriented edges `#1` to `#references`, each the edge
    /// element of the one before, and the vertex after the last.
    fn chain(references: usize) -> Vec<String> {
        let mut lines: Vec<String> = (1..=references)
            .map(|number| oriented_edge(number, number + 1))
            .collect();
        lines.push(format!("#{}=VERTEX('');", references + 1));
        lines
    }

    #[test]
    fn a_chain_at_the_bound_passes() {
        let lines = chain(LONGEST_CHAIN);

        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(checked(&lines), Ok(()));
    }

    #[test]
    fn a_chain_beyond_the_bound_is_refused_from_its_first_instance() {
        let lines = chain(LONGEST_CHAIN + 1);

        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(checked(&lines), Err(chain_message(1)));
    }

    #[test]
    fn a_chain_the_walk_reaches_by_a_short_path_first_is_measured_in_full() {
        // #1 refers to every edge, the last first. The stack never holds the long chain.
        let edges = LONGEST_CHAIN + 1;
        let list: Vec<String> = (2..edges + 2)
            .rev()
            .map(|number| format!("#{number}"))
            .collect();
        let mut lines = vec![format!("#1=PARTSMITH_LABEL(({}));", list.join(","))];
        lines.extend((2..edges + 2).map(|number| oriented_edge(number, number + 1)));
        lines.push(format!("#{}=VERTEX('');", edges + 2));

        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(checked(&lines), Err(chain_message(2)));
    }

    /// Runs `check` on a thread whose stack is 64 KiB. The WebAssembly build has no threads: there
    /// `check` runs on the one stack of the test binary.
    fn on_a_small_stack(
        check: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        if cfg!(target_os = "emscripten") {
            check()
        } else {
            std::thread::Builder::new()
                .stack_size(64 << 10)
                .spawn(check)
                .expect("the thread starts")
                .join()
                .expect("the check returns")
        }
    }

    #[test]
    fn a_long_cycle_is_refused_on_a_small_stack() {
        let edges = 100_000;
        let mut lines: Vec<String> = (2..edges + 2)
            .map(|number| oriented_edge(number, if number == edges + 1 { 2 } else { number + 1 }))
            .collect();
        lines.insert(0, "#1=EDGE_LOOP('',(#2));".to_owned());
        let text = step(&lines.iter().map(String::as_str).collect::<Vec<_>>());

        let result = on_a_small_stack(move || check_step_bytes(text.as_bytes()));

        assert_eq!(result, Err(cycle_message(2, edges)));
    }

    #[test]
    fn a_syntax_error_comes_before_a_cycle() {
        assert_eq!(
            checked(&[&oriented_edge(2, 2), "#3=PARTSMITH_LABEL(,);"]),
            Err(syntax_message(6))
        );
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        for bytes in [
            &b"#"[..],
            b"'",
            b"/*",
            b"#1=",
            b"(((((",
            b")))))",
            b"#1=X(#1",
            b"\xff\xfe#9(",
            b"ENDSCOPE ENDSCOPE &SCOPE",
            b"\"0",
            b"..",
            b"1.E",
        ] {
            let _ = check_step_bytes(bytes);
        }
    }
}
