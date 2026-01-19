file(Lines) :- getfile("Cargo.toml", Lines).


quoted(T) --> "\"","" , "\"".


text(N,'') --> text(N, ' ').
text(N, T) --> 
  { replace(T, '"', '\\"', NT) },
  format_("L~d: text \"~d: ~s\" ljust;", [N, N, NT]).

text_lines(_,[]) --> [].
text_lines(N,[H|T]) --> text(N,H), { NN is N + 1 }, text_lines(NN,T).

mark(Label) --> format_("left; move to ~a; move 10%;line left 30% color red; circle height 10% color red;", [Label]).

diagram --> { file(Lines) }, "down;", text_lines(1,Lines),
            mark('L1'), mark('L7'), mark('L11').
            