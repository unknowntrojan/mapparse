# mapparse

a utility I wrote to parse the map file included with the recent aimware self-leak.
there is also an IDAPython script to import the symbol information into IDA.

to do that, run `cargo test export`, and select the produced `output.idasym` when running the idapython script. ignore all the errors, those are from me not deduplicating symbols. you could either deduplicate in the code, or add a suffix to duplicate symbols if they have different rva's.
you obviously have to have the leak's dll loaded in ida.

i included the map file from the leak so you can fuck around with the parsing.

![After applying symbols](https://i.imgur.com/fY0u3qS.png)
