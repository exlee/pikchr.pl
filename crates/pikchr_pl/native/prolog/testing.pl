% vim: filetype=prolog
:- module(testing).
:- dynamic(test/1).
test(label_atom) :- phrase(label(a), "A"). 
test(label_string) :- phrase(label("abc"), "ABC").
test(label_replace) :- phrase(label('hello.pl'), "HELLO_PL").
test(attr) :- phrase(at(fill, red), "fill red").
test(attr_compound) :- phrase(at(fill(red)), "fill red").
test(group_string) :- phrase(group("abc", ase(box)), "ABC: [box\n]\n").
test(group_atom) :- phrase(group(def, ase(box)), "DEF: [box\n]\n").
test(group_label) :- phrase(group(label(ghi), ase(box)),  "GHI: [box\n]\n").

test_result(X) :-
  ( test(X)
  -> R = ok
  ;  R = failure
  ),
  format("~2+..~a~20|~t ~a~n", [X,R]).
tests :-
    format("~ntesting~n~n"),
    forall(clause(test(Name), _Body), test_result(Name)),
    nl.
