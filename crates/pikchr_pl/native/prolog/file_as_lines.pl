% vim: filetype=prolog

:- module(file_as_lines).

file_text(N,'') --> file_text(N, ' ').
file_text(N, T) --> 
  { replace(T, '"', '\\"', NT) },
  format_("L~d: text \"~d: ~s\" ljust~n", [N, N, NT]).

file_lines(_,[]) --> [].
file_lines(N,[H|T]) -->
  file_text(N,H), { NN is N + 1 },
  file_lines(NN,T).
