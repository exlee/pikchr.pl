box(L,A) --> label(L), ": ", as(box), space, as(rad), " 0.05 ".
input(L, A) --> box(L, A), " fill skyblue".
move_to(L, Dir) --> "move to ", label(L), ".", as(Dir), nl.
process(L, A) --> box(L, A), " height 300%", " width 130%", semicolon, text_inner(L, A), move_to(L, e).
process_small(L, A) --> box(L, A), " height 80%", " width 80%", semicolon, text_outer(L, A), move_to(L, e).
text_inner(At, Text) --> "text ", asq(Text), "below  at ", label(At), ".n", nl.
text_outer(At, Text) --> "text ", asq(Text), "above  at ", label(At), ".n + (0,0.05)", nl.

socket(L, Dir, T) --> "box at ", label(L), ".", as(Dir),
                      " width 20%", " fit", " fill white", semicolon,
                      "line from last box.s to last box.n invisible ", quoted(T), " aligned",
                      nl.
                      
entry(L, T) --> socket(L, w, T).
exit(L, T) --> socket(L,e,T).
diagram -->
  input(wasm, 'tpl.wasm'), nl,
  as(move), nl, 
  process(build, 'build process'),
  ase(move),
  process_small(wasm_compile, 'wasm compilation'),
  entry(wasm_compile, "wasm"),
  exit(wasm_compile, "wasmc").
  