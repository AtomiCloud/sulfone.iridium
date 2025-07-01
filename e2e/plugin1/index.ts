import { PluginOutput, StartPluginWithLambda } from '@atomicloud/cyan-sdk';
import fs from 'node:fs';
import path from 'node:path';

StartPluginWithLambda(async (input): Promise<PluginOutput> => {
  // Generate 10 random characters
  const randomChars = Array.from({ length: 10 }, () => {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    return chars.charAt(Math.floor(Math.random() * chars.length));
  }).join('');

  // Write the random characters to a file named 'plugin1'
  const filePath = path.join(input.directory, 'plugin1');

  // Ensure the directory exists
  fs.mkdirSync(path.dirname(filePath), { recursive: true });

  fs.writeFileSync(filePath, randomChars);

  console.log(`Created file 'plugin1' with random content: ${randomChars}`);
  return { directory: input.directory };
});
