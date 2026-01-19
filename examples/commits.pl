
file(Lines) :- getfile("Cargo.toml", Lines).

quoted(V) --> "\"", V, "\"".

commit(Ref, Title) --> 
    "O: [ oval ", quoted(Ref), "width 80%", " height 30%", " ];",
    "text at O.e + (0.4,0) ", quoted(Title), ";"
.

line --> "move to O.n;move 5%; line; move 5%;".

comment(T) --> "dot invis;",
               "line from O.nw go 30% heading 320 color orange <-;",
               "text ", quoted(T), "rjust", " small", ";",
               "move to last dot;".

diagram --> "margin=5pt;",
            "up;",
            commit("000abc", "First commit"),
            line,
            commit("111def", "Second commit"),
            comment("That one was great!"),
            line,
            commit("222ghi", "Third commit"),
            comment("This one? Not sure.").