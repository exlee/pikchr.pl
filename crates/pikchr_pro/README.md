# pikchr_pro (as in Pikchr Prolog)

## Usage

`pikchr_pro` is both library for integration (I'm using it myself for Editor/Previewer) and a CLI utility. CLI utility is as simple as it gets - it reads Prolog file on STDIN and outputs transformed diagram through Pikchr into STDOUT.

```
cat my_diagram.pl | pikchr_pro > output.svg
```

The only requirement is usage of `diagram//0` DCG definition, as it is starting point for the wrapper. Note that no Pikchr utilities are included, so everything has to be provided pretty much from scratch through DCG.

This means that it's not possible to escape learning oneself some Prolog (thankfully DCGs are one of the easiest features) or [Pikchr].

## Plead 

This project is a fruit of many years of research on various diagrams solutions and I believe it reached endgame.
## Rationale / Architecture

[Pikchr] has been my favorite diagramming language for the long time and Prolog is my pet language for even longer. One day I was researching ways of creating diagrams declaratively and crazy idea popped in my head. What if I used Definite Clause Grammars (DCGs) and then used them to generate Pikchr code. 

Initially I made attempt to integrate [Scryer Prolog] however it failed. It seems that Scryer panics quite hard when it gets unexpected input etc, which made it impossible to get "live latency" through it (not to mention it took the whole app with itself).

Then I found out about [Trealla Prolog] and decided to run it through WASM (for Safety and Profit). Initial warmup takes some time (that's why CLI takes approx. 1.5 seconds to complete) but later calls are quick to execute (thus potential future live watcher could actually be fast). Outside of the fact that it integrates nicely it is also a one awesome of the Prolog implementation (love error messages). 

## Pikchr / Prolog DCG 101

### Pikchr

Just skim [Pikchr: Pikchr User Manual](https://pikchr.org/home/doc/trunk/doc/userman.md) :-)

### Prolog

I don't want this to be introduction to Prolog, but more like simplified version of DCGs. Think about DCGs as a definition of the grammars that can either digest or output tokens (well formed DCGs can do both of those!). For `pikchr_pl` the subset you might be interested in is generation, so I'll skip straight to it:

```prolog
hello --> "Hello".
world --> "World".
space --> " ".

greeting --> hello, space, world.
```

Voilá, here's our greeting. Let's change it to greeting someone!

```prolog
greeting(Who) --> hello, space, Who, "!".
```

If you'd be running Prolog interpreter I'd advise something like `phrase(greeting("John"), Output), format("~s", [Output]).` but this exactly the part `pikchr_pl` is taking care for you (of course, assuming that you're generating a diagram :))

Note that LLMs are quite good at generating Prolog code, so you might want to learn with them, just remember to remind them that you want DCGs.
## Attribution

TODO, see files

## TODO

- [ ] Groom README.md
- [ ] More examples
## Examples

Note: Editor/Viewer is not part of this release.

![[grid.png]]
![[file.png]]
![[items.png]]
![[commits.png]]
![[circles.png]]
![[events.png]]

[Pikchr]: https://pikchr.org
[Trealla]: https://github.com/trealla-prolog/trealla
[Trealla Prolog]: https://github.com/trealla-prolog/trealla