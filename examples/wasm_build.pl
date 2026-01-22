box(L,A) --> label(L), ": ", as(box), space, as(rad), " 0.05 ".
circle(L, A) --> label(L), ": ", as(circle).
oval(L,A) --> label(L), ":", as(oval), " width 90% height 80%" .
input(L, A) --> oval(L, A), " fill skyblue".
output(L, A) --> oval(L, A), " fill orange".

move_to(L, Dir) --> "move to ", label(L), ".", as(Dir), nl.
process(L, A) --> box(L, A), " height 300%", " width 130%", semicolon, place_text:text_inner(L, A), move_to(L, e).
                      

diagram -->
  input(wasm, 'tpl.wasm'), nl,

  place_text:text_center(wasm, 'tpl.wasm'), nl,
  as(move), " 20%", nl,
  shapes:pipe(wasm_compile, 'wasm compilation', "wasm", "wasmc"),
  as(move), " 20%", nl,
  output(wasmc, 'tpl.wasmc'), nl,
  place_text:text_center(wasmc, 'tpl.wasmc'),
    
  nl,
  as(move), nl, 
  process(build, 'build process'),
  ase(move).
  
