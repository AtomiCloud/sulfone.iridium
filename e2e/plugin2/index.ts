import { PluginOutput, StartPluginWithLambda } from '@atomicloud/cyan-sdk';
import fs from 'node:fs';
import path from 'node:path';

StartPluginWithLambda(async (input): Promise<PluginOutput> => {
  // Generate 10 random characters
  const randomChars = Array.from({ length: 15 }, () => {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    return chars.charAt(Math.floor(Math.random() * chars.length));
  }).join('');

  // Write the random characters to a file named 'plugin2'
  const filePath = path.join(input.directory, 'plugin2');
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, randomChars);

  console.log(`Created file 'plugin2' with random content: ${randomChars}`);
  return { directory: input.directory };
});
