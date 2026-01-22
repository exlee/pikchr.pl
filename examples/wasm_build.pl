box(L,A) --> label(L), ": ", as(box), space, as(rad), " 0.05 ".
circle(L, A) --> label(L), ": ", as(circle).

input(L, A) --> circle(L, A), " fill skyblue".
output(L, A) --> circle(L, A), " fill orange".

move_to(L, Dir) --> "move to ", label(L), ".", as(Dir), nl.
process(L, A) --> box(L, A), " height 300%", " width 130%", semicolon, text_inner(L, A), move_to(L, e).
text_inner(At, Text) --> "dot invis;", "text ", asq(Text), "below  at ", label(At), ".n", ";move to last dot;", nl.
text_outer(At, Text) --> "text ", asq(Text), "above  at ", label(At), ".n + (0,0.05)", nl.
text_center(At, Text) -->
  "dot invis;",
  "text ", asq(Text), "at ", label(At), nl.

socket(L, Dir, T) --> "box at ", label(L), ".", as(Dir),
                      " width 20%", " fit", " fill white", semicolon,
                      "line from last box.s to last box.n invisible ", quoted(T), " aligned",
                      nl.

pipe(L, A) --> box(L, A), " height 80%", " width 80%",
               semicolon, text_outer(L, A),
               move_to(L, e).
                      
entry(L, T) --> socket(L, w, T).
exit(L, T) --> socket(L,e,T).
diagram -->
  group("WCP", (
    input(wasm, 'tpl.wasm'), nl,
    text_center(wasm, 'tpl.wasm'), nl,
    pipe(wasm_compile, 'wasm compilation'),
    as(move), " 10%", nl,
    output(wasmc, 'tpl.wasmc'), nl,
    text_center(wasmc, 'tpl.wasmc')
  )),
  as(move), nl, 
  process(build, 'build process'),
  ase(move),
  entry(wasm_compile, "wasm"),
  exit(wasm_compile, "wasmc").
  
