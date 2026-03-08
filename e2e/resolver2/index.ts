import { ResolverOutput, StartResolverWithLambda } from '@atomicloud/cyan-sdk';

StartResolverWithLambda(async (input): Promise<ResolverOutput> => {
  const paths = input.files.map(f => f.path);
  const uniquePaths = new Set(paths);
  if (uniquePaths.size !== 1) {
    throw new Error(`Expected all files to have the same path, got: ${[...uniquePaths].join(', ')}`);
  }

  console.log('input', JSON.stringify(input));

  const sorted = [...input.files].sort(
    (a, b) => a.origin.layer - b.origin.layer || a.origin.template.localeCompare(b.origin.template),
  );
  const merged = sorted.map(f => f.content).join('\n');

  return {
    path: paths[0],
    content: merged,
  };
});
