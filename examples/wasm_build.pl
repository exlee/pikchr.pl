box(L,A) --> label(L), ": ", as(box), space, as(rad), " 0.05 ".
circle(L, A) --> label(L), ": ", as(circle).
oval(L,A) --> label(L), ":", as(oval), " width 90% height 80%" .
input(L, A) --> oval(L, A), " fill skyblue".
output(L, A) --> oval(L, A), " fill orange".

move_to(L, Dir) --> "move to ", label(L), ".", as(Dir), nl.

pipe(L, A, Entry, Exit) --> box(L), " height 80%", " width 80%",
               semicolon, txt:text_outer(L, A),
               entry(L, Entry),
               exit(L, Exit),
               move_to(L, e).


wasm_compilation --> label(wasm_process), ": [",
                     
  input(wasm, 'tpl.wasm'), nl,

  txt:text_center(wasm, 'tpl.wasm'), nl,
  "dot invis;",
  "move 130%;",
  output(wasmc, 'tpl.wasmc'), nl,
  "move to last dot;",
  as(move), " 20%", nl,
  shapes:pipe(wasm_compile, 'wasm compilation', "wasm", "wasmc"),
  as(move), " 20%", nl,
  
  txt:text_center(wasmc, 'tpl.wasmc'),
  "]", ";move to ", label(wasm_process), ".end", nl.                                         

diagram -->
  group_start(everything),
  wasm_compilation, 
  box(pro, "Pikchr Pro"),
  group_end,
    
  as(move), nl, 
  process(build, everything, 'build process'),
  ase(move).
  
  
process(L, Around, A) -->
  containers:around(around, 0.2, 0.5, Around),
  txt:text_above(label(around), as(A)).
                          