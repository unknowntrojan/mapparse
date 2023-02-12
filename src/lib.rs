use anyhow::{Context, Result};

//
// This particular map file is composed like this:
//
// <name>
//
// Timestamp is <timestamp> (<human_readable_timestamp>)
//
// Preferred load address is <preferred_load>
//
// Start			Length		Name		Class
// <seg>:<addr>		<len>		<section>	<class>
// 0001:00000000	00003780H	.text		CODE
//
// Address			Publics by value	Rva+Base			Lib:Object
// <seg>:<addr>		<symbol>			<rva>		<flags>	<lib+obj>
// 0001:00000000	_lj_BC_ISLT			10001000	f		luajit-x86:lj_vm_x86.obj
//
// entry point at	<seg>:<addr>
//
// Static symbols
//
// <seg>:<addr>		<symbol>	<rva>	<flags>	<obj>

struct Rva(usize);

struct Address {
    seg: u16,
    addr: usize,
}

#[derive(Debug)]
enum Class {
    Code,
    Data,
}

struct Section<'a> {
    name: &'a str,
    class: Class,
    addr: Address,
    len: usize,
}

#[derive(Debug)]
enum LibObject<'a> {
    LibObj(Option<&'a str>, &'a str),
    Absolute,
}

struct Function<'a> {
    pub symbol: &'a str,
    pub addr: Address,
    pub rva: Rva,
    pub flags: Vec<&'a str>,
    pub libobj: LibObject<'a>,
}

struct StaticSymbol<'a> {
    pub symbol: &'a str,
    pub addr: Address,
    pub rva: Rva,
    pub flags: Vec<&'a str>,
    pub libobj: LibObject<'a>,
}

struct MapFile<'a> {
    pub file_name: &'a str,
    pub entrypoint: Address,
    pub preferred_load_addr: usize,
    pub timestamp: &'a str,
    pub sections: Vec<Section<'a>>,
    pub functions: Vec<Function<'a>>,
    pub static_symbols: Vec<StaticSymbol<'a>>,
}

impl<'a> MapFile<'a> {
    fn load(input: &'a str) -> Result<Self> {
        #[derive(Debug)]
        enum Stage {
            Header,
            Sections,
            Functions,
            StaticSymbols,
        }

        let mut stage = Stage::Header;

        let mut filename: Option<&str> = None;
        let mut timestamp: Option<&str> = None;
        let mut load_address: Option<usize> = None;
        let mut entry_point: Option<Address> = None;
        let mut sections: Vec<Section> = Default::default();
        let mut functions: Vec<Function> = Default::default();
        let mut static_symbols: Vec<StaticSymbol> = Default::default();

        for (line, data) in input.split("\r\n").enumerate() {
            // we are using zero-based indices, but i would like to use editor line numbers
            // using line numbers in general is yucky, but there is for example no clean way for me
            // to know which line the filename line is, as it does not contain anything else
            let line = line + 1;

            match stage {
                Stage::Header => match line {
                    1 => filename = Some(data.trim()),
                    3 => {
                        let begin = data.find('(').context("there was no timestamp on line 3")?;
                        let end = data.find(')').context("there was no timestamp on line 3")?;

                        timestamp = Some(&data[begin + 1..end - 1])
                    }
                    5 => {
                        load_address = Some(
                            usize::from_str_radix(
                                &data[data.find("is ").context(
                                    "there was no preferred load address statement on line 5",
                                )? + 3..],
                                16,
                            )
                            .context("unable to get preferred load address from line 5")?,
                        )
                    }
                    7 => stage = Stage::Sections,
                    _ => {}
                },
                Stage::Sections => {
                    if data.contains("Publics by Value") {
                        stage = Stage::Functions;
                        continue;
                    }

                    // hacky way to know we are on an actual data line
                    if !data.contains('0') {
                        continue;
                    }

                    enum SectionStage {
                        Address,
                        Length,
                        Symbol,
                        Class,
                    }

                    let mut section_stage = SectionStage::Address;

                    let mut address: Option<Address> = None;
                    let mut length: Option<usize> = None;
                    let mut symbol: Option<&str> = None;
                    let mut class: Option<Class> = None;

                    for substring in data.split(' ') {
                        if substring.is_empty() {
                            continue;
                        }

                        match section_stage {
                            SectionStage::Address => {
                                let addrstr: Vec<&str> = substring.split(':').collect();

                                // these will panic if the format is invalid
                                let seg = addrstr[0];
                                let addr = addrstr[1];

                                address = Some(Address {
                                    seg: seg.parse().context("unable to parse segment")?,
                                    addr: usize::from_str_radix(addr, 16)
                                        .context("unable to parse address")?,
                                });

                                section_stage = SectionStage::Length;
                            }
                            SectionStage::Length => {
                                length = Some(
                                    usize::from_str_radix(&substring[0..substring.len() - 1], 16)
                                        .context("unable to parse length")?,
                                );

                                section_stage = SectionStage::Symbol;
                            }
                            SectionStage::Symbol => {
                                symbol = Some(substring);

                                section_stage = SectionStage::Class;
                            }
                            SectionStage::Class => {
                                class = Some(match substring {
                                    "CODE" => Class::Code,
                                    "DATA" => Class::Data,
                                    _ => {
                                        panic!("unrecognized section class {}", substring);
                                    }
                                });
                            }
                        }
                    }

                    sections.push(Section {
                        addr: address.context("no address was found")?,
                        len: length.context("no length was found")?,
                        name: symbol.context("no symbol was found")?,
                        class: class.context("no class was found")?,
                    })
                }
                Stage::Functions => {
                    if data.contains("entry point at") {
                        stage = Stage::StaticSymbols;

                        for substring in data.split(' ') {
                            if substring.is_empty() {
                                continue;
                            }

                            if substring.contains('0') {
                                let addrstr: Vec<&str> = substring.split(':').collect();

                                // these will panic if the format is invalid
                                let seg = addrstr[0];
                                let addr = addrstr[1];

                                entry_point = Some(Address {
                                    seg: seg.parse().context("unable to parse segment")?,
                                    addr: usize::from_str_radix(addr, 16)
                                        .context("unable to parse address")?,
                                });
                            }
                        }

                        continue;
                    }

                    // hacky way to know we are on an actual data line
                    if !data.contains('0') {
                        continue;
                    }

                    enum FunctionStage {
                        Address,
                        Symbol,
                        Rva,
                        LibObj,
                    }

                    let mut function_stage = FunctionStage::Address;
                    let mut address: Option<Address> = None;
                    let mut symbol: Option<&str> = None;
                    let mut rva: Option<Rva> = None;
                    let mut flags: Vec<&str> = Default::default();
                    let mut libobj: Option<LibObject> = None;

                    for substring in data.split(' ') {
                        if substring.is_empty() {
                            continue;
                        }

                        match function_stage {
                            FunctionStage::Address => {
                                let addrstr: Vec<&str> = substring.split(':').collect();

                                // these will panic if the format is invalid
                                let seg = addrstr[0];
                                let addr = addrstr[1];

                                address = Some(Address {
                                    seg: seg.parse().context("unable to parse segment")?,
                                    addr: usize::from_str_radix(addr, 16)
                                        .context("unable to parse address")?,
                                });

                                function_stage = FunctionStage::Symbol;
                            }
                            FunctionStage::Symbol => {
                                symbol = Some(substring);
                                function_stage = FunctionStage::Rva
                            }
                            FunctionStage::Rva => {
                                let rva_with_base = usize::from_str_radix(substring, 16)
                                    .context("unable to parse rva")?;

                                let val = if rva_with_base == 0 {
                                    0
                                } else {
                                    rva_with_base - load_address.unwrap()
                                };

                                rva = Some(Rva(val));
                                function_stage = FunctionStage::LibObj;
                            }
                            FunctionStage::LibObj => {
                                match substring.contains("<absolute>") {
                                    true => libobj = Some(LibObject::Absolute),
                                    false => {
                                        // this is code responsible for both LibObj and flags cases.
                                        // this is a bit retarded, but we can't have a flag state,
                                        // as we would need to switch match cases which isn't possible
                                        // as we don't have goto.
                                        match substring.len() {
                                            1 => {
                                                // FLAG!
                                                flags.push(substring)
                                            }
                                            _ => {
                                                let libobjstr: Vec<&str> =
                                                    substring.split(':').collect();

                                                match libobjstr.len() {
                                                    1 => {
                                                        libobj = Some(LibObject::LibObj(
                                                            None,
                                                            libobjstr[0],
                                                        ))
                                                    }
                                                    _ => {
                                                        libobj = Some(LibObject::LibObj(
                                                            Some(libobjstr[0]),
                                                            libobjstr[1],
                                                        ))
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    functions.push(Function {
                        addr: address.context("no address was found")?,
                        symbol: symbol.context("no symbol was found")?,
                        rva: rva.context("no rva was found")?,
                        flags,
                        libobj: libobj.context("no libobj was found")?,
                    })
                }
                Stage::StaticSymbols => {
                    // reused code from function stage

                    // hacky way to know we are on an actual data line
                    if !data.contains('0') {
                        continue;
                    }

                    enum FunctionStage {
                        Address,
                        Symbol,
                        Rva,
                        LibObj,
                    }

                    let mut function_stage = FunctionStage::Address;
                    let mut address: Option<Address> = None;
                    let mut symbol: Option<&str> = None;
                    let mut rva: Option<Rva> = None;
                    let mut flags: Vec<&str> = Default::default();
                    let mut libobj: Option<LibObject> = None;

                    for substring in data.split(' ') {
                        if substring.is_empty() {
                            continue;
                        }

                        match function_stage {
                            FunctionStage::Address => {
                                let addrstr: Vec<&str> = substring.split(':').collect();

                                // these will panic if the format is invalid
                                let seg = addrstr[0];
                                let addr = addrstr[1];

                                address = Some(Address {
                                    seg: seg.parse().context("unable to parse segment")?,
                                    addr: usize::from_str_radix(addr, 16)
                                        .context("unable to parse address")?,
                                });

                                function_stage = FunctionStage::Symbol;
                            }
                            FunctionStage::Symbol => {
                                symbol = Some(substring);
                                function_stage = FunctionStage::Rva
                            }
                            FunctionStage::Rva => {
                                let rva_with_base = usize::from_str_radix(substring, 16)
                                    .context("unable to parse rva")?;

                                let val = if rva_with_base == 0 {
                                    0
                                } else {
                                    rva_with_base - load_address.unwrap()
                                };

                                rva = Some(Rva(val));
                                function_stage = FunctionStage::LibObj;
                            }
                            FunctionStage::LibObj => {
                                match substring.contains("<absolute>") {
                                    true => libobj = Some(LibObject::Absolute),
                                    false => {
                                        // this is code responsible for both LibObj and flags cases.
                                        // this is a bit retarded, but we can't have a flag state,
                                        // as we would need to switch match cases which isn't possible
                                        // as we don't have goto.
                                        match substring.len() {
                                            1 => {
                                                // FLAG!
                                                flags.push(substring)
                                            }
                                            _ => {
                                                if substring.len() < 3 {
                                                    dbg!(substring.len());
                                                }

                                                let libobjstr: Vec<&str> =
                                                    substring.split(':').collect();

                                                match libobjstr.len() {
                                                    1 => {
                                                        libobj = Some(LibObject::LibObj(
                                                            None,
                                                            libobjstr[0],
                                                        ))
                                                    }
                                                    _ => {
                                                        libobj = Some(LibObject::LibObj(
                                                            Some(libobjstr[0]),
                                                            libobjstr[1],
                                                        ))
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    static_symbols.push(StaticSymbol {
                        addr: address.context("no address was found")?,
                        symbol: symbol.context("no symbol was found")?,
                        rva: rva.context("no rva was found")?,
                        flags,
                        libobj: libobj.context("no libobj was found")?,
                    })
                }
            }
        }

        Ok(MapFile {
            file_name: filename.context("filename not found")?,
            entrypoint: entry_point.context("entrypoint not found")?,
            preferred_load_addr: load_address.context("preferred load address not found")?,
            timestamp: timestamp.context("timestamp not found")?,
            sections,
            functions,
            static_symbols,
        })
    }
}

#[test]
fn parse() {
    let map_data = std::fs::read("csgo-x86.map").unwrap();
    let map_string = String::from_utf8(map_data).unwrap();

    let map = MapFile::load(&map_string).unwrap();

    println!("Dumping map for object file {}, entry point ({}:{:#04X}), preferred load addr {:#04X}, built on {}", map.file_name, map.entrypoint.seg, map.entrypoint.addr, map.preferred_load_addr, map.timestamp);

    for section in &map.sections {
        println!(
            "{:?} Section {}, Segment {}, Address {}, Length {}",
            section.class, section.name, section.addr.seg, section.addr.addr, section.len
        )
    }

    for function in &map.functions {
        println!(
            "Function {} at rva {:#04X} ({}:{:#04X}) with flags {:?} in {:?}",
            function.symbol,
            function.rva.0,
            function.addr.seg,
            function.addr.addr,
            function.flags,
            function.libobj
        )
    }

    for symbol in &map.static_symbols {
        println!(
            "Static Symbol {} at rva {:#04X} ({}:{:#04X}) with flags {:?} in {:?}",
            symbol.symbol,
            symbol.rva.0,
            symbol.addr.seg,
            symbol.addr.addr,
            symbol.flags,
            symbol.libobj
        )
    }
}

#[test]
fn export() {
    fn fix_name_for_ida(name: &str) -> String {
        name.chars()
            .map(|x| {
                match "_$?@0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxy"
                    .contains(x)
                {
                    true => x,
                    false => '_',
                }
            })
            .collect()
    }

    let map_data = std::fs::read("csgo-x86.map").unwrap();
    let map_string = String::from_utf8(map_data).unwrap();

    let map = MapFile::load(&map_string).unwrap();

    let mut output: String = Default::default();
    let flags = msvc_demangler::DemangleFlags::NAME_ONLY;

    for function in &map.functions {
        output.push_str(
            format!(
                "{} {}\n",
                function.rva.0 + map.preferred_load_addr,
                fix_name_for_ida(
                    &msvc_demangler::demangle(function.symbol, flags)
                        .unwrap_or(function.symbol.to_owned())
                )
            )
            .as_str(),
        );
    }

    for symbol in &map.static_symbols {
        output.push_str(
            format!(
                "{} {}\n",
                symbol.rva.0 + map.preferred_load_addr,
                fix_name_for_ida(
                    &msvc_demangler::demangle(symbol.symbol, flags)
                        .unwrap_or(symbol.symbol.to_owned())
                )
            )
            .as_str(),
        );
    }

    std::fs::write("output.idasym", output).unwrap();
}
