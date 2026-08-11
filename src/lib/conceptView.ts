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

import type { ConceptGraph, ConceptNode, Graph, GraphNode } from "./api";

/** A concept graph plus what a click on one of its nodes should reveal. */
export interface ConceptView {
  graph: Graph;
  /** Synthetic node path → the term behind it. */
  details: Map<string, ConceptNode>;
}

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
 * How a note path is shown in the list a term opens.
 *
 * Splits on both separators because a Windows vault reports backslashes, and
 * strips the extension case-insensitively. If anything about a path defeats
 * all that, the raw path is shown rather than an empty row — a blank line is
 * a bug that hides itself, and this list rendered blank once already.
 */
export function noteLabel(path: string): string {
  const base = path.split(/[\\/]/).filter(Boolean).pop() ?? "";
  return base.replace(/\.md$/i, "") || path;
}

/**
 * Map a concept graph onto the note-graph shape the renderer consumes.
 *
 * `fallbackName` supplies a translated "Topic N" for the rare cluster whose
 * own terms give no usable name.
 */
export function conceptGraphToGraph(
  concepts: ConceptGraph,
  fallbackName: (index: number) => string
): ConceptView {
  // A group named after its biggest word beats "Topic 4". Two groups can end
  // up wanting the same word, so a repeat gets the number appended rather than
  // two identical legend entries.
  const used = new Map<string, number>();
  const clusterName = (index: number): string => {
    const raw = concepts.clusterNames?.[index]?.trim();
    if (!raw) return fallbackName(index);
    const seen = used.get(raw) ?? 0;
    used.set(raw, seen + 1);
    return seen === 0 ? raw : `${raw} (${seen + 1})`;
  };
  const tiers = sizeTiers(concepts.nodes.map((n) => n.weight));
  const pathOf = new Map<string, string>();
  const details = new Map<string, ConceptNode>();
  const names = new Map<number, string>();
  const nameOf = (cluster: number): string => {
    let name = names.get(cluster);
    if (name === undefined) {
      name = clusterName(cluster);
      names.set(cluster, name);
    }
    return name;
  };

  const nodes: GraphNode[] = concepts.nodes.map((node, i) => {
    // The renderer groups by the text before the last slash, so the cluster
    // name there earns colours and a legend entry with no renderer change.
    const path = `${nameOf(node.cluster)}/${node.id}`;
    pathOf.set(node.id, path);
    details.set(path, node);
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

  return { graph: { nodes, edges }, details };
}
