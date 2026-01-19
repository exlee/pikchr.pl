sc --> ";".
item(Name) --> 
  { fill_for(Name, Fill) },
  format_("oval \"~s\" fill ~w;", [Name, Fill]).

arrow --> "arrow", sc.
move --> "move", sc.
dot --> "dot", sc.

fill_for("Apple", red).
fill_for("Pear", green).
fill_for("Banana", yellow).

fill_for(_, white).

apple --> item("Apple").
pear --> item("Pear").
banana --> item("Banana").

move_little --> "move 20%;".

diagram -->
        "down;",
	apple,
        move_little,
        pear,
        move_little,
        banana.