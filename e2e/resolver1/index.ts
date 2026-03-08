import { ResolverOutput, StartResolverWithLambda } from '@atomicloud/cyan-sdk';

StartResolverWithLambda(async (input): Promise<ResolverOutput> => {
  // In a real resolver, this would perform actual deep merge
  console.log(`Resolving conflicts for: ${input.files}`);
  // Return the merged content (in a real resolver, this would perform actual merge)
  return {
    content: 'heh',
    path: 'random',
  };
});
