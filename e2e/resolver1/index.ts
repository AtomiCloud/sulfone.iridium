import { ResolverOutput, StartResolverWithLambda } from '@atomicloud/cyan-sdk';

StartResolverWithLambda(async (input): Promise<ResolverOutput> => {
  // Simple JSON merger resolver - performs deep merge
  console.log(`Resolving conflicts for: ${input.filePath}`);

  // Return the merged content (in a real resolver, this would perform actual merge)
  return {
    content: input.content,
    success: true,
  };
});
