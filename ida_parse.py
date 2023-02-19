# todo: add prim type ident

import idc;
import ida_kernwin;
 
def update_function_name(ident, rva):
    func_name = idc.get_func_name(rva)
    if func_name[:3] == "sub":
        idc.set_name(rva, ident, 0);

def parse(line):
    parts = line.split(' ')
    rva = int(parts[0])
    symbol = parts[1][:-1]
    mangled = parts[2][:-1]

    if rva != 0:
        update_function_name(mangled, rva)


fileName = ida_kernwin.ask_file(0, "*.sym", "mapparse exported file");
if fileName:
    ida_kernwin.msg("Open File\n");
    mapFile = open(fileName, "r");
    for line in mapFile.readlines():
        parse(line);
    mapFile.close();
    ida_kernwin.msg("Now, go to Options->Demangled names and select 'Names'!\n");
else:
    ida_kernwin.msg("No file selected!\n");