% vim: filetype=prolog
:- module(shapes).

socket(L, Dir, T) --> "box at ", label(L), ".", as(Dir),
                      " width 20%", " fit", " fill white", semicolon,
                      "line from last box.s to last box.n invisible ", quoted(T), " aligned",
                      nl.
entry(L, T) --> socket(L,w,T).
exit(L, T) --> socket(L,e,T).
box(L) --> label(L), ": ", as(box), space, as(rad), " 0.05 ".
move_to(L, Dir) --> "move to ", label(L), ".", as(Dir), nl. 
pipe(L, A, Entry, Exit) --> box(L), " height 80%", " width 80%",
               semicolon, txt:text_outer(L, A),
               entry(L, Entry),
               exit(L, Exit),
               move_to(L, e).
