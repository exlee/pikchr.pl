% vim: filetype=prolog
:- use_module(library(format)).
:- use_module(library(dcgs)).

:- module(runtime).
run :- phrase(query:diagram, Out), format("~s", [Out]).

:- module(query).
