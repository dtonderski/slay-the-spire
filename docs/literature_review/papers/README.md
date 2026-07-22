# Privileged-Search Combat RL Reading List

This folder contains the initial reading set for an AlphaZero-style Slay the
Spire combat agent. The policy/value network consumes fair public information;
its bootstrap search is privileged and follows the one true authoritative
simulator state. Training episodes start from collected combat roots rather
than from a two-player game's standard opening position.

## Suggested reading order

1. **Kocsis and Szepesvari (2006), _Bandit Based Monte-Carlo Planning_**
   - File: `Kocsis+Szepesvari_2006.pdf`
   - Introduces UCT and the exploration/exploitation rule underlying classic
     MCTS.
   - Source: <https://aima.eecs.berkeley.edu/~russell/classes/cs294/s11/readings/Kocsis%2BSzepesvari%3A2006.pdf>

2. **Anthony, Tian, and Barber (2017), _Thinking Fast and Slow with Deep
   Learning and Tree Search_**
   - File: `NIPS-2017-thinking-fast-and-slow-with-deep-learning-and-tree-search-Paper.pdf`
   - Introduces Expert Iteration: tree search acts as the expert and a neural
     policy acts as the apprentice, with each improving the other.
   - Source: <https://papers.neurips.cc/paper_files/paper/2017/file/d8e1344e27a5b08cdfd5d027d9b8d6de-Paper.pdf>

3. **Silver et al. (2017), _Mastering the Game of Go without Human
   Knowledge_**
   - File: `Silver_et_al_2017_AlphaGo_Zero.pdf`
   - The clearest presentation of the AlphaGo Zero training loop, PUCT search,
     visit-count policy targets, and terminal value targets.
   - Source: <https://discovery.ucl.ac.uk/id/eprint/10045895/1/agz_unformatted_nature.pdf>

4. **Silver et al. (2017), _Mastering Chess and Shogi by Self-Play with a
   General Reinforcement Learning Algorithm_**
   - File: `1712.01815v1.pdf`
   - Generalizes the AlphaGo Zero recipe into AlphaZero and illustrates
     legal-action masking and domain-specific action encodings.
   - Source: <https://arxiv.org/pdf/1712.01815>

5. **Danihelka et al. (2022), _Policy Improvement by Planning with Gumbel_**
   - File: `Danihelka_et_al_2022_Gumbel_AlphaZero.pdf`
   - Introduces Gumbel AlphaZero, which is especially relevant when only a
     small number of simulations can be afforded per decision.
   - Source: <https://arxiv.org/pdf/2207.10075>

6. **Grill et al. (2020), _Monte-Carlo Tree Search as Regularized Policy
   Optimization_**
   - File: `Grill_et_al_2020_MCTS_as_Regularized_Policy_Optimization.pdf`
   - Interprets neural MCTS as a policy-improvement operator and clarifies why
     the network is trained toward the search policy.
   - Source: <https://proceedings.mlr.press/v119/grill20a/grill20a.pdf>

7. **Browne et al. (2012), _A Survey of Monte Carlo Tree Search Methods_**
   - File: `Browne_et_al_2012_MCTS_Survey.pdf`
   - Reference for transpositions, tree reuse, rollout policies, progressive
     bias, and parallel search.
   - Source: <https://www.lamsade.dauphine.fr/~cazenave/A%2BSurvey%2Bof%2BMonte%2BCarlo%2BTree%2BSearch%2BMethods.pdf>

## Mapping to this project

| Literature term | Slay the Spire combat agent |
|---|---|
| Board position | Fair combat observation and public history for the network; full snapshot only inside privileged search |
| Legal moves | Variable public player-choice list mapped by Rust to authoritative actions |
| Self-play game | Simulator combat episode |
| Starting position | Sampled combat root |
| Policy target | Root search visit distribution |
| Value target | Terminal combat outcome |
| Policy/value network | Priors and leaf values for combat search |

Unlike AlphaZero's two-player games, combat has one decision-maker. Value
backups must therefore not negate the value on alternating tree depths. The
initial value is a versioned handcrafted terminal proxy where survival
dominates HP, max-HP, gold, and exact remaining-potion preferences. There is no
hard potion budget in the learned-agent design.

The later fair-search phase replaces the single privileged root with a belief
over hidden states while reusing the fair network, public choice scorer, and
training infrastructure. POMCP, BetaZero, and particle-filter literature belong
to that later search stage.
