import { PluginOutput, StartPluginWithLambda } from '@atomicloud/cyan-sdk';
import fs from 'node:fs';
import path from 'node:path';

StartPluginWithLambda(async (input): Promise<PluginOutput> => {
  // Write fixed deterministic content to a file named 'plugin2' for snapshot testing
  const filePath = path.join(input.directory, 'plugin2');
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, 'xxxxxxxxxxxxxxx\n');

  console.log(`Created file 'plugin2' with deterministic content`);
  return { directory: input.directory };
});
