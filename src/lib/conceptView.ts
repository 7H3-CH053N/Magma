// Turning a concept graph into something the existing graph renderer can draw.
//
// The renderer already knows how to lay out, colour, size and label a graph of
// notes. A concept graph is the same shape carrying different meanings, so
// rather than growing a second renderer it is mapped onto the first one:
//
//   cluster  →  the "folder" that decides colour and legend entry
//   term     →  the note
//   how often the term is written  →  the size tier
//
// The two graphs are never drawn at once, so the synthetic paths here cannot
// collide with real ones and need no marker. What they do need is that nothing
// tries to *open* them: the view knows which mode it is in and does not offer
// a concept node to the editor.

import type { ConceptGraph, Graph, GraphNode } from "./api";

/**
 * Split terms into the same five size steps the note view uses.
 *
 * By rank, not by raw count: a vault where one word appears 900 times and the
 * rest 20 would otherwise render as one big dot and 149 identical specks.
 * Ranking keeps the picture readable whatever the distribution, at the cost of
 * the sizes being relative — which is what they are in the note view too.
 */
function sizeTiers(weights: number[]): Map<number, number> {
  const order = weights
    .map((w, i) => [w, i] as const)
    .sort((a, b) => b[0] - a[0] || a[1] - b[1]);
  const tiers = new Map<number, number>();
  order.forEach(([, index], rank) => {
    const share = order.length <= 1 ? 0 : rank / (order.length - 1);
    // The head of the ranking is tier 4, the long tail is tier 0.
    const tier = share < 0.05 ? 4 : share < 0.15 ? 3 : share < 0.35 ? 2 : share < 0.7 ? 1 : 0;
    tiers.set(index, tier);
  });
  return tiers;
}

/**
 * Map a concept graph onto the note-graph shape the renderer consumes.
 *
 * `clusterName` is supplied by the caller so the legend can be translated —
 * the cluster labels are read by a person, not by code.
 */
export function conceptGraphToGraph(
  concepts: ConceptGraph,
  clusterName: (index: number) => string
): Graph {
  const tiers = sizeTiers(concepts.nodes.map((n) => n.weight));
  const pathOf = new Map<string, string>();

  const nodes: GraphNode[] = concepts.nodes.map((node, i) => {
    // The renderer groups by the text before the last slash, so the cluster
    // name there earns colours and a legend entry with no renderer change.
    const path = `${clusterName(node.cluster)}/${node.id}`;
    pathOf.set(node.id, path);
    return {
      path,
      title: node.label,
      aiAuthored: false,
      degree: node.noteCount,
      missing: false,
      sizeTier: tiers.get(i) ?? 0,
    };
  });

  const edges = concepts.edges
    .map((e) => ({ source: pathOf.get(e.source), target: pathOf.get(e.target) }))
    .filter((e): e is { source: string; target: string } => !!e.source && !!e.target);

  return { nodes, edges };
}
