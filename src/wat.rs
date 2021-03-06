use std::str;
use std::char;
use std::result;
use lexer::{WatLexer, WatToken, WatTokenType, WatPosition};

#[derive(Debug,Copy,Clone)]
pub struct WatParserError {
    pub message: &'static str,
    pub line: usize,
    pub column: usize,
}

pub type Result<T> = result::Result<T, WatParserError>;

pub type Keyword = Vec<u8>;
pub type Data = Vec<u8>;
pub type ID = Vec<u8>;
pub type OptionalID = Option<ID>;
pub type Name = String;

#[derive(Debug)]
pub struct WatLimits {
    pub min: u32,
    pub max: Option<u32>,
}

#[derive(Debug)]
pub struct WatMemoryType {
    pub limits: WatLimits,
    pub shared: bool,
}

#[derive(Debug)]
pub enum WatTableType {

}

#[derive(Debug)]
pub enum WatValType {
    I32,
    I64,
    F32,
    F64,
}

#[derive(Debug)]
pub struct WatParam {
    pub id: OptionalID,
    pub valtype: WatValType,
}

#[derive(Debug)]
pub struct WatResult {
    pub valtype: WatValType,
}

#[derive(Debug)]
pub struct WatLocal {
    pub id: OptionalID,
    pub valtype: WatValType,
}

#[derive(Debug,Clone,Copy)]
pub enum WatSign {
    Positive,
    Negative,
}

#[derive(Debug,Clone)]
pub enum WatFloat {
    Number(WatSign, Data, i32),
    NaN(WatSign, Option<Data>),
    Inf(WatSign),
}

#[derive(Debug)]
pub enum WatInstructionArg {
    ID(ID),
    Unsigned(Data),
    Signed(WatSign, Data),
    Float(WatFloat),
    Flags(Keyword, u32),
}

#[derive(Debug)]
pub struct WatTypeuse {
    pub id: OptionalID,
    pub params: Vec<WatParam>,
    pub results: Vec<WatResult>,
}

impl WatTypeuse {
    fn empty() -> WatTypeuse {
        WatTypeuse {
            id: None,
            params: vec![],
            results: vec![],
        }
    }
}

#[derive(Debug)]
pub enum WatGlobalType {

}

#[derive(Debug)]
pub enum WatImport {
    Func { id: OptionalID, typeuse: WatTypeuse },
    Table {
        id: OptionalID,
        tabletype: WatTableType,
    },
    Memory {
        id: OptionalID,
        memtype: WatMemoryType,
    },
    Global {
        id: OptionalID,
        globaltype: WatGlobalType,
    },
}

fn parse_hexnum_u32(bytes: &[u8]) -> Option<u32> {
    // FIXME '_'?
    let num = str::from_utf8(bytes);
    if num.is_err() {
        return None;
    }
    u32::from_str_radix(num.unwrap(), 16).ok()
}

fn parse_u32(bytes: &[u8]) -> Option<u32> {
    if bytes.len() > 2 && bytes[0] == b'0' && bytes[0] == b'x' {
        return parse_hexnum_u32(&bytes[2..]);
    }
    let num = str::from_utf8(bytes);
    if num.is_err() {
        return None;
    }
    num.unwrap().parse::<u32>().ok()
}

fn convert_u32_to_data(maybe_num: Option<u32>) -> Option<Data> {
    if maybe_num.is_none() {
        return None;
    }
    let mut result = Vec::new();
    let mut num = maybe_num.unwrap();
    result.push((num & 0xFF) as u8);
    while num >= 0x100 {
        num >>= 8;
        result.push((num & 0xFF) as u8);
    }
    Some(result)
}

fn parse_hexnum(bytes: &[u8]) -> Option<Data> {
    assert!(bytes.len() > 0);
    if bytes.len() <= 8 {
        return convert_u32_to_data(parse_hexnum_u32(bytes));
    }
    unimplemented!(); // FIXME
}

fn parse_num(bytes: &[u8]) -> Option<Data> {
    if bytes.len() > 2 && bytes[0] == b'0' && bytes[0] == b'x' {
        return parse_hexnum(&bytes[2..]);
    }
    assert!(bytes.len() > 0);
    if bytes.len() <= 9 {
        return convert_u32_to_data(parse_u32(bytes));
    }
    unimplemented!(); // FIXME
}

fn parse_float(bytes: &[u8]) -> Option<(WatSign, Data, i32)> {
    Some((WatSign::Positive, vec![], 0)) // FIXME
}

fn parse_string(bytes: &[u8]) -> String {
    assert!(bytes.len() >= 2 && bytes[0] == b'\"' && bytes[bytes.len() - 1] == b'\"');
    let mut i = 1;
    let last = bytes.len() - 1;
    let mut result = Vec::new();
    while i < last {
        let ch = bytes[i];
        i += 1;
        if ch != b'\\' {
            result.push(ch);
            continue;
        }
        let escape = bytes[i];
        i += 1;
        match escape {
            b't' => result.push(0x09),
            b'n' => result.push(0x0A),
            b'r' => result.push(0x0D),
            b'\"' => result.push(b'\"'),
            b'\'' => result.push(b'\''),
            b'\\' => result.push(b'\\'),
            b'u' => {
                if bytes[i] != b'{' {
                    panic!();
                }
                i += 1;
                let j = i;
                while bytes[i] != b'}' {
                    i += 1;
                }
                let hexnum = parse_hexnum_u32(&bytes[j..i]).unwrap(); // FIXME
                let code = char::from_u32(hexnum).unwrap(); // FIXME
                let mut buffer = [0; 5];
                let code_bytes = code.encode_utf8(&mut buffer).as_bytes();
                result.extend_from_slice(&code_bytes);
                assert!(i < last);
                i += 1;
            }
            _ => panic!(),
        }
    }
    String::from_utf8(result).unwrap()
}

#[derive(Debug)]
pub enum WatParserState {
    Initial,
    End,
    Error(WatParserError),
    StartModule { id: OptionalID },
    EndModule,
    Import {
        modname: Name,
        fieldname: Name,
        import: WatImport,
    },
    StartFunc {
        id: OptionalID,
        export_name: Option<Name>,
        typeuse: WatTypeuse,
        locals: Vec<WatLocal>,
    },
    EndFunc,
    CodeOperator {
        instruction: Keyword,
        args: Vec<WatInstructionArg>,
        group: bool,
        position: WatPosition,
    },
    CodeOperatorEnd,
}

enum KnownKeyword {
    Func,
    Import,
    Memory,
    Shared,
}

pub struct WatParser<'a> {
    lexer: WatLexer<'a>,
    state: WatParserState,
    func_depth: Option<u32>,
}

impl<'a> WatParser<'a> {
    pub fn new(source: &[u8]) -> WatParser {
        return WatParser {
                   lexer: WatLexer::new(source),
                   state: WatParserState::Initial,
                   func_depth: None,
               };
    }

    fn current_token(&self) -> &WatToken {
        self.lexer.current_token()
    }

    fn current_token_type(&self) -> &WatTokenType {
        &self.lexer.current_token().ty
    }

    fn current_token_content(&self) -> &[u8] {
        self.lexer.current_token_content()
    }

    fn create_error(&self, message: &'static str) -> WatParserError {
        let ref position = self.current_token().start;
        WatParserError {
            message,
            line: position.line,
            column: position.column,
        }
    }

    fn advance(&mut self) -> Result<()> {
        let result = self.lexer.next();
        if result.is_ok() {
            return Ok(());
        }
        let err = result.unwrap_err();
        Err(WatParserError {
                message: err.message,
                line: err.line,
                column: err.column,
            })
    }

    fn rewind_token(&mut self) {
        self.lexer.rewind();
    }

    fn maybe_open_paren(&mut self) -> Result<bool> {
        if let WatTokenType::OpenParen = *self.current_token_type() {
            self.advance()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn expect_open_paren(&mut self) -> Result<()> {
        if self.maybe_open_paren()? {
            return Ok(());
        }
        Err(self.create_error("( is expected"))
    }

    fn maybe_close_paren(&mut self) -> Result<bool> {
        if let WatTokenType::CloseParen = *self.current_token_type() {
            self.advance()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn expect_close_paren(&mut self) -> Result<()> {
        if self.maybe_close_paren()? {
            return Ok(());
        }
        Err(self.create_error(") is expected"))
    }

    fn maybe_exact_keyword(&mut self, keyword: &[u8]) -> Result<bool> {
        if let WatTokenType::Keyword = *self.current_token_type() {
            if self.current_token_content() == keyword {
                self.advance()?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn expect_exact_keyword(&mut self, keyword: &[u8]) -> Result<()> {
        if self.maybe_exact_keyword(keyword)? {
            return Ok(());
        }
        Err(self.create_error("?? keyword is expected"))
    }

    fn is_keyword(&self) -> bool {
        if let WatTokenType::Keyword = *self.current_token_type() {
            true
        } else {
            false
        }
    }

    fn get_keyword(&self) -> Result<&[u8]> {
        if self.is_keyword() {
            return Ok(self.current_token_content());
        }
        Err(self.create_error("a keyword is expected"))
    }

    fn is_memarg_flag(&self) -> Result<bool> {
        let content = self.get_keyword()?;
        Ok(content.len() > 7 && &content[..7] == b"offset=" ||
           content.len() > 6 && &content[..6] == b"flags=")
    }

    fn maybe_id(&mut self) -> Result<OptionalID> {
        if let WatTokenType::ID = *self.current_token_type() {
            let id = Vec::from(self.current_token_content());
            self.advance()?;
            return Ok(Some(id));
        }
        Ok(None)
    }

    fn read_id(&mut self) -> Result<ID> {
        let id = self.maybe_id()?;
        if id.is_some() {
            return Ok(id.unwrap());
        }
        Err(self.create_error("id is expected"))
    }

    fn read_u32(&mut self) -> Result<u32> {
        if let WatTokenType::Unsigned = *self.current_token_type() {
            let result = {
                let result = parse_u32(self.current_token_content());
                if result.is_none() {
                    return Err(self.create_error("unable to read u32"));
                }
                result.unwrap()
            };
            self.advance()?;
            return Ok(result);
        }
        unreachable!();
    }

    fn read_name(&mut self) -> Result<Name> {
        if let WatTokenType::String = *self.current_token_type() {
            let name = parse_string(self.current_token_content());
            self.advance()?;
            return Ok(name);
        }
        unreachable!();
    }

    fn read_keyword(&mut self) -> Result<Keyword> {
        if let WatTokenType::Keyword = *self.current_token_type() {
            let keyword = Vec::from(self.current_token_content());
            self.advance()?;
            return Ok(keyword);
        }
        unreachable!();
    }

    fn read_limits(&mut self) -> Result<WatLimits> {
        let min = self.read_u32()?;
        let max = if let WatTokenType::Unsigned = *self.current_token_type() {
            Some(self.read_u32()?)
        } else {
            None
        };
        Ok(WatLimits { min, max })
    }

    fn read_memtype(&mut self) -> Result<WatMemoryType> {
        if self.maybe_open_paren()? {
            self.expect_exact_keyword(b"shared")?;
            let limits = self.read_limits()?;
            self.expect_close_paren()?;
            return Ok(WatMemoryType {
                          limits,
                          shared: true,
                      });
        }
        let limits = self.read_limits()?;
        Ok(WatMemoryType {
               limits,
               shared: false,
           })
    }

    fn read_start_module(&mut self) -> Result<()> {
        self.advance()?;
        self.expect_open_paren()?;
        self.expect_exact_keyword(b"module")?;
        let id = self.maybe_id()?;
        self.state = WatParserState::StartModule { id };
        Ok(())
    }

    fn read_memory_import(&mut self) -> Result<WatImport> {
        self.advance()?;
        let id = self.maybe_id()?;
        let memtype = self.read_memtype()?;
        Ok(WatImport::Memory { id, memtype })
    }

    fn read_import(&mut self) -> Result<()> {
        self.advance()?;
        let modname = self.read_name()?;
        let fieldname = self.read_name()?;
        self.expect_open_paren()?;
        let keyword = match self.get_keyword()? {
            b"memory" => KnownKeyword::Memory,
            _ => unimplemented!("nyi"),
        };
        let import = match keyword {
            KnownKeyword::Memory => self.read_memory_import()?,
            _ => panic!(),
        };
        self.expect_close_paren()?;

        self.state = WatParserState::Import {
            modname,
            fieldname,
            import,
        };
        self.expect_close_paren()?;
        Ok(())
    }

    fn read_valtype(&mut self) -> Result<WatValType> {
        let valtype = match self.get_keyword()? {
            b"i32" => WatValType::I32,
            b"f64" => WatValType::I64,
            b"f32" => WatValType::F32,
            b"f64" => WatValType::F64,
            _ => unimplemented!("nyi"),
        };
        self.advance()?;
        Ok(valtype)
    }

    fn read_typeuse_after_open_paren(&mut self) -> Result<(WatTypeuse, bool)> {
        let mut id = None;
        if self.maybe_exact_keyword(b"type")? {
            id = self.maybe_id()?;
            if id.is_none() {
                return Err(self.create_error("id is expected for typeuse"));
            }
            self.expect_close_paren()?;
            if !self.maybe_open_paren()? {
                return Ok((WatTypeuse {
                               id,
                               params: vec![],
                               results: vec![],
                           },
                           false));
            }
        }
        let mut params = Vec::new();
        while self.maybe_exact_keyword(b"param")? {
            let param_id = self.maybe_id()?;
            let no_id = param_id.is_none();
            let valtype = self.read_valtype()?;
            params.push(WatParam {
                            id: param_id,
                            valtype,
                        });
            while no_id && self.is_keyword() {
                let valtype = self.read_valtype()?;
                params.push(WatParam { id: None, valtype });
            }
            self.expect_close_paren()?;
            if !self.maybe_open_paren()? {
                return Ok((WatTypeuse {
                               id,
                               params,
                               results: vec![],
                           },
                           false));
            }
        }
        let mut results = Vec::new();
        while self.maybe_exact_keyword(b"result")? {
            let valtype = self.read_valtype()?;
            results.push(WatResult { valtype });
            while self.is_keyword() {
                let valtype = self.read_valtype()?;
                results.push(WatResult { valtype });
            }
            self.expect_close_paren()?;
            if !self.maybe_open_paren()? {
                return Ok((WatTypeuse {
                               id,
                               params,
                               results,
                           },
                           false));
            }
        }
        Ok((WatTypeuse {
                id,
                params,
                results,
            },
            true))
    }

    fn read_typeuse(&mut self) -> Result<WatTypeuse> {
        if self.maybe_open_paren()? {
            let (typeuse, keyword_expected) = self.read_typeuse_after_open_paren()?;
            if keyword_expected {
                self.rewind_token();
            }
            return Ok(typeuse);
        }
        Ok(WatTypeuse::empty())
    }

    fn read_locals_after_open_paren(&mut self) -> Result<(Vec<WatLocal>, bool)> {
        let mut locals = Vec::new();
        while self.maybe_exact_keyword(b"local")? {
            let id = self.maybe_id()?;
            let no_id = id.is_none();
            let valtype = self.read_valtype()?;
            locals.push(WatLocal { id, valtype });
            while no_id && self.is_keyword() {
                let valtype = self.read_valtype()?;
                locals.push(WatLocal { id: None, valtype });
            }
            self.expect_close_paren()?;
            if !self.maybe_open_paren()? {
                return Ok((locals, false));
            }
        }
        Ok((locals, true))
    }

    fn read_func(&mut self) -> Result<()> {
        self.advance()?;
        let id = self.maybe_id()?;
        let (export_name, typeuse, locals) = if self.maybe_open_paren()? {
            if self.maybe_exact_keyword(b"import")? {
                let modname = self.read_name()?;
                let fieldname = self.read_name()?;
                self.expect_close_paren()?;
                let typeuse = self.read_typeuse()?;
                self.expect_close_paren()?;
                let import = WatImport::Func { id, typeuse };
                self.state = WatParserState::Import {
                    modname,
                    fieldname,
                    import,
                };
                return Ok(());
            }
            let export_name = if self.maybe_exact_keyword(b"export")? {
                let name = Some(self.read_name()?);
                self.expect_close_paren()?;
                if !self.maybe_open_paren()? {
                    self.state = WatParserState::StartFunc {
                        id,
                        export_name: name,
                        typeuse: WatTypeuse::empty(),
                        locals: vec![],
                    };
                    return Ok(());
                }
                name
            } else {
                None
            };
            let (typeuse, keyword_expected) = self.read_typeuse_after_open_paren()?;
            if keyword_expected {
                let (locals, keyword_expected) = self.read_locals_after_open_paren()?;
                if keyword_expected {
                    self.rewind_token();
                }
                (export_name, typeuse, locals)
            } else {
                (export_name, typeuse, vec![])
            }
        } else {
            (None, WatTypeuse::empty(), vec![])
        };
        self.state = WatParserState::StartFunc {
            id,
            export_name,
            typeuse,
            locals,
        };
        self.func_depth = Some(0);
        Ok(())
    }

    fn read_memarg_flag(&mut self) -> Result<WatInstructionArg> {
        let keyword = self.read_keyword()?;
        Ok(WatInstructionArg::Flags(keyword, 0)) // FIXME
    }

    fn read_arg_id(&mut self) -> Result<WatInstructionArg> {
        let id = self.read_id()?;
        Ok(WatInstructionArg::ID(id))
    }

    fn read_arg_signed(&mut self) -> Result<WatInstructionArg> {
        let (sign, data) = {
            let signed = self.current_token_content();
            assert!(signed[0] == b'-' || signed[0] == b'+');
            let sign = match signed[0] {
                b'-' => WatSign::Negative,
                b'+' => WatSign::Positive,
                _ => unreachable!(),
            };
            let data = parse_num(&signed[1..]);
            (sign, data)
        };
        if data.is_none() {
            return Err(self.create_error("Unable to parse signed"));
        }
        self.advance()?;
        Ok(WatInstructionArg::Signed(sign, data.unwrap()))
    }

    fn read_arg_unsigned(&mut self) -> Result<WatInstructionArg> {
        let data = parse_num(self.current_token_content());
        if data.is_none() {
            return Err(self.create_error("Unable to parse unsigned"));
        }
        self.advance()?;
        Ok(WatInstructionArg::Unsigned(data.unwrap()))
    }

    fn read_arg_float(&mut self) -> Result<WatInstructionArg> {
        let result = parse_float(self.current_token_content());
        if result.is_none() {
            return Err(self.create_error("Unable to parse float"));
        }
        self.advance()?;
        let (sign, data, power) = result.unwrap();
        Ok(WatInstructionArg::Float(WatFloat::Number(sign, data, power)))
    }

    fn read_func_body(&mut self) -> Result<()> {
        if self.maybe_close_paren()? {
            if self.func_depth.unwrap() == 0 {
                self.state = WatParserState::EndFunc;
                self.func_depth = None;
                return Ok(());
            }
            self.state = WatParserState::CodeOperatorEnd;
            self.func_depth = Some(self.func_depth.unwrap() - 1);
            return Ok(());
        }
        let group = if self.maybe_open_paren()? {
            true
        } else {
            false
        };
        let position = self.current_token().start;
        let instruction = self.read_keyword()?;
        let mut args = Vec::new();
        'main: loop {
            match *self.current_token_type() {
                WatTokenType::End => break,
                WatTokenType::Keyword => {
                    if self.is_memarg_flag()? {
                        args.push(self.read_memarg_flag()?);
                        continue;
                    }
                    break 'main;
                }
                WatTokenType::OpenParen | WatTokenType::CloseParen => {
                    break 'main;
                }
                WatTokenType::ID => {
                    args.push(self.read_arg_id()?);
                }
                WatTokenType::Signed => {
                    args.push(self.read_arg_signed()?);
                }
                WatTokenType::Unsigned => {
                    args.push(self.read_arg_unsigned()?);
                }
                WatTokenType::Float => {
                    args.push(self.read_arg_float()?);
                }
                _ => {
                    return Err(self.create_error("unexpected token in the instruction"));
                }
            }
        }
        if group {
            self.func_depth = Some(self.func_depth.unwrap() + 1);
        }
        self.state = WatParserState::CodeOperator {
            instruction,
            args,
            group,
            position,
        };
        Ok(())
    }

    fn read_module_field(&mut self) -> Result<()> {
        if self.maybe_close_paren()? {
            self.state = WatParserState::EndModule;
            return Ok(());
        }
        self.expect_open_paren()?;
        let keyword = match self.get_keyword()? {
            b"import" => KnownKeyword::Import,
            b"func" => KnownKeyword::Func,
            _ => unreachable!("nyi"),
        };
        match keyword {
            KnownKeyword::Import => self.read_import(),
            KnownKeyword::Func => self.read_func(),
            _ => panic!(),
        }
    }

    fn find_end(&mut self) -> Result<()> {
        if let WatTokenType::End = *self.current_token_type() {
            self.state = WatParserState::End;
            return Ok(());
        }
        Err(self.create_error("unexpected content after the module"))
    }

    pub fn parse(&mut self) -> &WatParserState {
        let result = match self.state {
            WatParserState::End => panic!("WatParser at the end of stream"),
            WatParserState::Error(_) => panic!("WatParser in error state"),
            WatParserState::EndModule => self.find_end(),
            WatParserState::Initial => self.read_start_module(),
            WatParserState::StartModule { .. } |
            WatParserState::EndFunc |
            WatParserState::Import { .. } => self.read_module_field(),
            WatParserState::StartFunc { .. } |
            WatParserState::CodeOperator { .. } |
            WatParserState::CodeOperatorEnd => self.read_func_body(),
            _ => panic!("nyi"),
        };
        if result.is_err() {
            self.state = WatParserState::Error(result.unwrap_err());
        }
        &self.state
    }
}
