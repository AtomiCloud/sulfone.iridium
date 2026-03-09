import { ResolverOutput, StartResolverWithLambda } from '@atomicloud/cyan-sdk';
import { createMerger } from 'smob';

StartResolverWithLambda(async (input): Promise<ResolverOutput> => {
  const paths = input.files.map(f => f.path);
  const uniquePaths = new Set(paths);
  if (uniquePaths.size !== 1)
    throw new Error(`Expected all files to have the same path, got: ${[...uniquePaths].join(', ')}`);

  const sorted = [...input.files].sort(
    (a, b) => a.origin.layer - b.origin.layer || a.origin.template.localeCompare(b.origin.template),
  );

  const strat = input.config.arrayStrategy as 'concat' | 'replace' | 'distinct';
  if (!['concat', 'replace', 'distinct'].includes(strat)) {
    throw new Error(`arrayStrategy has to be concat, replace and distinct`);
  }

  let cfg;
  if (strat === 'concat') {
    cfg = {
      array: true,
      arrayDistinct: false,
    };
  } else if (strat === 'replace') {
    cfg = {
      array: false,
      arrayDistinct: false,
    };
  } else {
    cfg = {
      array: true,
      arrayDistinct: true,
    };
  }

  const merge = createMerger(cfg);
  const merged = merge(...sorted.map(x => JSON.parse(x.content)));

  return {
    content: JSON.stringify(merged),
    path: paths[0],
  };
});
